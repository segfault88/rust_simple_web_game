

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

sudo groupadd --system caddy
sudo useradd --system \
    --gid caddy \
    --create-home \
    --home-dir /var/lib/caddy \
    --shell /usr/sbin/nologin \
    --comment "Caddy web server" \
    caddy

vim /etc/security/pwquality.conf
dictcheck = 0


```