# shellcheck disable=SC2034
# archiso profile definition for the NULLXES live + installer ISO.

iso_name="nullxes"
iso_label="NULLXES_$(date +%Y%m)"
iso_publisher="NULLXES OS <https://nullxes.os>"
iso_application="NULLXES OS Live Installer"
iso_version="$(date +%Y.%m.%d)"

install_dir="nullxes"
buildmodes=('iso')
bootmodes=('uefi.systemd-boot')
arch="x86_64"
pacman_conf="pacman.conf"
airootfs_image_type="erofs"
airootfs_image_tool_options=('-zlz4hc,12' '-T0')
file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/root"]="0:0:750"
  ["/root/.automated_script.sh"]="0:0:755"
  ["/usr/local/bin/Installer_Entry"]="0:0:755"
)
