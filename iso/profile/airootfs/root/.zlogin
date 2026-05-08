# Live-ISO autostart: bring up greetd in autologin mode pointing at FRAME.
# greetd reads /etc/greetd/config.toml; on the live ISO we replace it with
# the autologin variant during airootfs assembly.
if [[ "$XDG_VTNR" == "1" && -z "$DISPLAY" && -z "$WAYLAND_DISPLAY" ]]; then
    sudo systemctl start greetd.service
fi
