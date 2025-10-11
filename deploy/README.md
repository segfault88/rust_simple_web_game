

```
apt update && apt upgrade
apt install whois fish btop

ufw allow 2222/tcp
ufw allow 80/tcp
ufw allow 443/tcp

useradd -m <user>
mkpasswd -m sha-512 <password>

<user> ALL=(ALL) NOPASSWD: ALL


chsh -s /usr/bin/fish

vim /etc/ssh/sshd_config

PermitRootLogin no
Port 2222
PasswordAuthentication no
PermitEmptyPasswords no


useradd -m caddy -s /usr/sbin/nologin


# This allows non-root users to bind to ports 80 and higher.

sudo sh -c 'echo "net.ipv4.ip_unprivileged_port_start=80" >> /etc/sysctl.d/99-custom-settings.conf'
sudo sysctl --system | grep unprivileged

```