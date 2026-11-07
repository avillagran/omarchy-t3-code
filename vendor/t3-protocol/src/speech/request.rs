//! Abortable HTTP voice operations. The caller owns the async runtime and the
//! consent/authority lifetime; no hidden thread, automatic retry or microphone.
use super::*;
use std::{
    future::{poll_fn, Future},
    pin::{pin, Pin},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
};
use zeroize::Zeroize;

struct CancellationState {
    cancelled: AtomicBool,
    waiters: Mutex<Vec<Option<Waker>>>,
}
/// Permanent, cloneable cancellation. Dropping an in-flight operation drops its
/// actual reqwest future and response; waiting for this token never sends data.
#[derive(Clone)]
pub struct RequestCancellation(Arc<CancellationState>);
impl Default for RequestCancellation {
    fn default() -> Self {
        Self(Arc::new(CancellationState {
            cancelled: AtomicBool::new(false),
            waiters: Mutex::new(vec![]),
        }))
    }
}
impl RequestCancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        let waiters = self
            .0
            .waiters
            .lock()
            .map(|mut values| std::mem::take(&mut *values))
            .unwrap_or_default();
        for waiter in waiters.into_iter().flatten() {
            waiter.wake();
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    async fn run<F: Future>(&self, future: F) -> std::result::Result<F::Output, RequestFailure> {
        let mut cancelled = pin!(Cancelled {
            token: self,
            slot: None
        });
        let mut work = pin!(future);
        poll_fn(|context| {
            if cancelled.as_mut().poll(context).is_ready() {
                return Poll::Ready(Err(RequestFailure::Cancelled));
            }
            work.as_mut().poll(context).map(Ok)
        })
        .await
    }
}
struct Cancelled<'a> {
    token: &'a RequestCancellation,
    slot: Option<usize>,
}
impl Future for Cancelled<'_> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        let token = self.token;
        let Ok(mut waiters) = token.0.waiters.lock() else {
            return Poll::Ready(());
        };
        if token.is_cancelled() {
            return Poll::Ready(());
        }
        let index = match self.slot {
            Some(index) => index,
            None => {
                let index = waiters
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(waiters.len());
                if index == waiters.len() {
                    waiters.push(None);
                }
                self.slot = Some(index);
                index
            }
        };
        if !waiters[index]
            .as_ref()
            .is_some_and(|waker| waker.will_wake(context.waker()))
        {
            waiters[index] = Some(context.waker().clone());
        }
        Poll::Pending
    }
}
impl Drop for Cancelled<'_> {
    fn drop(&mut self) {
        if let (Some(index), Ok(mut waiters)) = (self.slot, self.token.0.waiters.lock()) {
            if let Some(slot) = waiters.get_mut(index) {
                *slot = None;
            }
            while waiters.last().is_some_and(Option::is_none) {
                waiters.pop();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestFailure {
    Cancelled,
    ConsentRevoked,
    Transport(String),
    Control(ControlFailure),
    Speech(SpeechFailure),
}
pub type RequestResult<T> = std::result::Result<T, RequestFailure>;
fn transport(value: &str) -> RequestFailure {
    RequestFailure::Transport(value.into())
}
fn admitted(cancel: &RequestCancellation, can_send: &impl Fn() -> bool) -> RequestResult<()> {
    if cancel.is_cancelled() {
        Err(RequestFailure::Cancelled)
    } else {
        let allowed = can_send();
        if cancel.is_cancelled() {
            Err(RequestFailure::Cancelled)
        } else if !allowed {
            Err(RequestFailure::ConsentRevoked)
        } else {
            Ok(())
        }
    }
}
/// Same-origin voice HTTP with native per-route deadlines and explicit abort.
/// The caller must cancel when its owner/consent changes. No request begins
/// until its async method is polled; no credential or audio implements Debug.
pub struct CancellableSpeech {
    http: reqwest::Client,
    base: reqwest::Url,
    credential: crate::Credential,
}
enum ResponseKind {
    Control,
    EmptyControl,
    Speech,
}
struct Request<'a> {
    method: &'static str,
    path: &'a str,
    body: Option<Vec<u8>>,
    binary: bool,
    response: ResponseKind,
}
// The HTTP driver can retain a request body after its response future is
// cancelled. Keep the actual producer on the same token, with bounded frames
// and cooperative polls; abandoning only the response cannot stop an upload.
struct RequestBody {
    bytes: zeroize::Zeroizing<Vec<u8>>,
    offset: usize,
    cancelled: RequestCancellation,
    yield_next: bool,
    failed: bool,
}
// A driver may outlive the response future. Retire only this operation's body
// when its future is dropped; sibling operations may share the public token.
struct RetireBody(RequestCancellation);
impl Drop for RetireBody {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl http_body::Body for RequestBody {
    type Data = bytes::Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<http_body::Frame<Self::Data>, Self::Error>>> {
        if self.failed {
            return Poll::Ready(None);
        }
        if self.cancelled.is_cancelled() {
            self.bytes.zeroize();
            self.failed = true;
            return Poll::Ready(Some(Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Voice request cancelled",
            ))));
        }
        if self.offset == self.bytes.len() {
            return Poll::Ready(None);
        }
        if self.yield_next {
            self.yield_next = false;
            context.waker().wake_by_ref();
            return Poll::Pending;
        }
        let end = self.bytes.len().min(self.offset + 32 * 1024);
        let bytes = bytes::Bytes::copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        self.yield_next = true;
        Poll::Ready(Some(Ok(http_body::Frame::data(bytes))))
    }
    fn is_end_stream(&self) -> bool {
        self.failed || self.offset == self.bytes.len()
    }
    fn size_hint(&self) -> http_body::SizeHint {
        http_body::SizeHint::with_exact(self.bytes.len().saturating_sub(self.offset) as u64)
    }
}
impl Connection {
    pub fn cancellable_speech(&self) -> Result<CancellableSpeech> {
        Ok(CancellableSpeech {
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .build()
                .map_err(|_| "local_error")?,
            base: self.base.clone(),
            credential: self.credential.clone(),
        })
    }
}
impl CancellableSpeech {
    /// Fresh session for this exact write credential. The host checks its
    /// required scope before capture; a different read identity cannot grant it.
    pub async fn session(
        &self,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<Value> {
        self.exchange(
            Request {
                method: "GET",
                path: "/api/auth/session",
                body: None,
                binary: false,
                response: ResponseKind::Control,
            },
            cancel,
            &can_send,
        )
        .await
    }
    pub async fn cloud_keys(
        &self,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<CloudKeys> {
        let value = self
            .exchange(
                Request {
                    method: "GET",
                    path: "/api/voice/cloud/keys",
                    body: None,
                    binary: false,
                    response: ResponseKind::Speech,
                },
                cancel,
                &can_send,
            )
            .await?;
        decode_cloud_keys(&value).map_err(RequestFailure::Speech)
    }
    async fn exchange(
        &self,
        input: Request<'_>,
        cancel: &RequestCancellation,
        can_send: &impl Fn() -> bool,
    ) -> RequestResult<Value> {
        admitted(cancel, can_send)?;
        let body_lifetime = RetireBody(RequestCancellation::default());
        let url = endpoint(&self.base, input.path).map_err(RequestFailure::Transport)?;
        let method = reqwest::Method::from_bytes(input.method.as_bytes())
            .map_err(|_| transport("invalid_method"))?;
        let mut request = self
            .http
            .request(method.clone(), url.clone())
            .timeout(request_timeout(input.path))
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity");
        for (name, value) in self
            .credential
            .headers(method.as_str(), url.as_str())
            .map_err(RequestFailure::Transport)?
        {
            request = request.header(name, value);
        }
        if let Some(body) = input.body {
            request = request
                .header(
                    "Content-Type",
                    if input.binary {
                        "application/octet-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(reqwest::Body::wrap(RequestBody {
                    bytes: zeroize::Zeroizing::new(body),
                    offset: 0,
                    cancelled: body_lifetime.0.clone(),
                    yield_next: false,
                    failed: false,
                }));
        }
        let transaction = async {
            let mut response = request.send().await.map_err(|_| transport("unreachable"))?;
            let status = response.status().as_u16();
            let control = !matches!(input.response, ResponseKind::Speech);
            let special = if control {
                status == 400
            } else {
                status == 422
            };
            if !special && !(200..300).contains(&status) {
                return Err(if status == 401 || status == 403 {
                    transport("unauthorized")
                } else {
                    transport(&format!("http_{status}"))
                });
            }
            if !special && matches!(input.response, ResponseKind::EmptyControl) {
                return Ok(Value::Null);
            }
            let media = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if (control || !special) && media != "application/json" {
                return Err(transport(if special { "http_400" } else { "not_json" }));
            }
            if (control || !special)
                && response
                    .headers()
                    .get("content-encoding")
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| {
                        !value.is_empty() && !value.eq_ignore_ascii_case("identity")
                    })
            {
                return Err(transport("unsupported_encoding"));
            }
            let limit = if special { 65536 } else { 8 * 1024 * 1024 };
            let mut bytes = zeroize::Zeroizing::new(Vec::new());
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| transport("unreachable"))?
            {
                if bytes.len() + chunk.len() > limit {
                    return Err(transport("too_large"));
                }
                bytes.extend_from_slice(&chunk);
            }
            if special {
                return if control {
                    Err(RequestFailure::Control(controls::invalid_request_payload(
                        &bytes,
                    )))
                } else {
                    cloud_error(&bytes).map_err(RequestFailure::Speech)
                };
            }
            serde_json::from_slice(&bytes).map_err(|_| transport("malformed"))
        };
        let result = cancel.run(transaction).await?;
        admitted(cancel, can_send)?;
        result
    }
    pub async fn status(
        &self,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<Value> {
        self.exchange(
            Request {
                method: "GET",
                path: "/api/voice/status",
                body: None,
                binary: false,
                response: ResponseKind::Control,
            },
            cancel,
            &can_send,
        )
        .await
    }
    pub async fn prepare(
        &self,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<()> {
        self.exchange(
            Request {
                method: "POST",
                path: "/api/voice/models/prepare",
                body: None,
                binary: false,
                response: ResponseKind::EmptyControl,
            },
            cancel,
            &can_send,
        )
        .await
        .map(|_| ())
    }
    pub async fn transcribe(
        &self,
        pcm: &[u8],
        options: &TranscriptionOptions,
        cloud: Option<(CloudProvider, Option<&str>)>,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<String> {
        admitted(cancel, &can_send)?;
        let body = encode_pcm_request(pcm, options).map_err(RequestFailure::Transport)?;
        let mut path = cloud
            .map(|(provider, _)| format!("/api/voice/cloud/{}/transcribe", provider.as_str()))
            .unwrap_or_else(|| "/api/voice/transcribe".into());
        if let Some((_, Some(model))) = cloud {
            if !model_id(model) {
                return Err(transport("invalid_speech_model"));
            }
            let mut url = endpoint(&self.base, &path).map_err(RequestFailure::Transport)?;
            url.query_pairs_mut().append_pair("model", model);
            path = format!("{}?{}", url.path(), url.query().unwrap());
        }
        let value = self
            .exchange(
                Request {
                    method: "POST",
                    path: &path,
                    body: Some(body),
                    binary: true,
                    response: ResponseKind::Speech,
                },
                cancel,
                &can_send,
            )
            .await?;
        value["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| transport("malformed"))
    }
    pub async fn post_process(
        &self,
        input: &PostProcessRequest,
        cancel: &RequestCancellation,
        can_send: impl Fn() -> bool,
    ) -> RequestResult<String> {
        let value = self
            .exchange(
                Request {
                    method: "POST",
                    path: "/api/voice/post-process",
                    body: Some(input.as_json().to_string().into_bytes()),
                    binary: false,
                    response: ResponseKind::Control,
                },
                cancel,
                &can_send,
            )
            .await?;
        value["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| transport("malformed"))
    }
}
