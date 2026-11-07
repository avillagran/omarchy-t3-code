use omarchy_t3_client::browser_automation::*;
fn source(environment: &str, profile: &str, guest: &str) -> Source {
    Source {
        environment: environment.into(),
        profile: profile.into(),
        guest: guest.into(),
    }
}
fn inspector(source: &Source, thread: &str, tab: &str, instance: &str) -> Inspector {
    Inspector {
        source: source.clone(),
        thread: thread.into(),
        tab: tab.into(),
        instance: instance.into(),
    }
}
fn admit(registry: &mut Registry, inspector: &Inspector, target: &str) -> Lease {
    registry.expect_inspector(inspector).unwrap();
    registry
        .admit(TargetProof::verify(inspector.clone(), target, target).unwrap())
        .unwrap()
}
#[test]
fn matching_target_observation_and_current_inspector_are_required() {
    let mut registry = Registry::default();
    let source = source("env", "default", "actual-child");
    registry.register_guest(&source);
    let view = inspector(&source, "thread", "tab", "actual-inspector");
    assert!(TargetProof::verify(view.clone(), "expected", "different").is_err());
    assert!(registry
        .admit(TargetProof::verify(view.clone(), "target", "target").unwrap())
        .is_err());
    let lease = admit(&mut registry, &view, "target");
    assert!(registry.accepts(&lease));
    let repeated = registry
        .admit(TargetProof::verify(view, "target", "target").unwrap())
        .unwrap();
    assert_eq!(lease.id(), repeated.id());
}
#[test]
fn delayed_probe_error_and_retirement_cannot_replace_new_inspector() {
    let mut registry = Registry::default();
    let source = source("env", "default", "actual-child");
    registry.register_guest(&source);
    let old = inspector(&source, "thread", "tab", "old-inspector");
    let old_lease = admit(&mut registry, &old, "old-target");
    let late = TargetProof::verify(old.clone(), "old-target", "old-target").unwrap();
    let next = inspector(&source, "thread", "tab", "new-inspector");
    assert_eq!(
        registry.expect_inspector(&next).unwrap().unwrap().id(),
        old_lease.id()
    );
    let new_lease = admit(&mut registry, &next, "new-target");
    assert!(registry.admit(late).is_err());
    assert!(registry.retire_inspector(&old).is_none());
    assert!(registry.retire_lease(&old_lease).is_none());
    assert!(registry.accepts(&new_lease));
}
#[test]
fn late_guest_failure_does_not_retire_replacement_or_other_environment() {
    let mut registry = Registry::default();
    let old = source("env", "default", "old-child");
    let other = source("other", "default", "other-child");
    registry.register_guest(&old);
    registry.register_guest(&other);
    let old_view = inspector(&old, "thread", "tab", "old-inspector");
    let old_lease = admit(&mut registry, &old_view, "same-target");
    let other_lease = admit(
        &mut registry,
        &inspector(&other, "thread", "tab", "other-inspector"),
        "same-target",
    );
    let new = source("env", "default", "new-child");
    let retired = registry.register_guest(&new);
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].id(), old_lease.id());
    let new_lease = admit(
        &mut registry,
        &inspector(&new, "thread", "tab", "new-inspector"),
        "same-target",
    );
    assert!(registry.retire_guest(&old).is_empty());
    assert!(registry.expect_inspector(&old_view).is_err());
    assert!(registry.accepts(&new_lease));
    assert!(registry.accepts(&other_lease));
}
#[test]
fn tuple_scope_keys_separate_profiles_threads_and_embedded_delimiters() {
    let mut registry = Registry::default();
    let a = source("env", "profile:one", "child-a");
    let b = source("env", "profile", "child-b");
    registry.register_guest(&a);
    registry.register_guest(&b);
    let first = admit(
        &mut registry,
        &inspector(&a, "a\0b", "c", "first"),
        "target-a",
    );
    let second = admit(
        &mut registry,
        &inspector(&a, "a", "b\0c", "second"),
        "target-b",
    );
    let profile = admit(
        &mut registry,
        &inspector(&b, "a\0b", "c", "third"),
        "target-a",
    );
    assert_eq!(registry.current(&a, "a\0b", "c").unwrap().id(), first.id());
    assert_eq!(registry.current(&a, "a", "b\0c").unwrap().id(), second.id());
    assert_eq!(
        registry.current(&b, "a\0b", "c").unwrap().id(),
        profile.id()
    );
    assert!(registry.current(&a, "other", "c").is_none());
}
#[test]
fn one_physical_target_cannot_be_admitted_to_another_tab() {
    let mut registry = Registry::default();
    let source = source("env", "default", "actual-child");
    registry.register_guest(&source);
    let first = admit(
        &mut registry,
        &inspector(&source, "thread", "one", "first"),
        "actual-target",
    );
    let other = inspector(&source, "other-thread", "two", "second");
    registry.expect_inspector(&other).unwrap();
    assert!(matches!(
        registry.admit(TargetProof::verify(other, "actual-target", "actual-target").unwrap()),
        Err("browser_target_already_owned")
    ));
    assert!(registry.accepts(&first));
}

#[test]
fn retired_attachment_cannot_be_resurrected_by_its_delayed_probe() {
    let mut registry = Registry::default();
    let source = source("env", "default", "actual-child");
    registry.register_guest(&source);
    let view = inspector(&source, "thread", "tab", "actual-inspector");
    let lease = admit(&mut registry, &view, "target");
    let delayed = TargetProof::verify(view, "target", "target").unwrap();
    assert_eq!(registry.retire_lease(&lease).unwrap().id(), lease.id());
    assert!(!registry.accepts(&lease));
    assert!(registry.admit(delayed).is_err());
}

#[test]
fn an_existing_inspector_cannot_silently_retarget_its_lease() {
    let mut registry = Registry::default();
    let source = source("env", "default", "actual-child");
    registry.register_guest(&source);
    let view = inspector(&source, "thread", "tab", "actual-inspector");
    let lease = admit(&mut registry, &view, "target");
    assert!(matches!(
        registry.admit(TargetProof::verify(view, "another-target", "another-target").unwrap()),
        Err("browser_target_mismatch")
    ));
    assert!(registry.accepts(&lease));
}
