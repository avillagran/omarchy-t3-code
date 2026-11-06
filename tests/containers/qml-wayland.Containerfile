FROM docker.io/library/archlinux:latest
# Keep an isolated compositor runtime when the runner mounts host Qt read-only.
RUN pacman -Sy --noconfirm sway \
    && mkdir -p /opt/sway-runtime \
    && cp -a /usr /opt/sway-runtime/ \
    && setcap -r /opt/sway-runtime/usr/bin/sway
