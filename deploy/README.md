

```
apt update && apt upgrade
apt install whois fish btop podman podman-compose

ufw allow 2222/tcp
ufw allow 80/tcp
ufw allow 443/tcp

chsh -s /usr/bin/fish


# This allows non-root users to bind to ports 80 and higher.

sudo sh -c 'echo "net.ipv4.ip_unprivileged_port_start=80" >> /etc/sysctl.d/99-custom-settings.conf'
sudo sysctl --system | grep unprivileged

```