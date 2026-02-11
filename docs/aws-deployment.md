# AWS Deployment Guide

Deploy GeoPop API on an Ubuntu EC2 instance with Docker, connect to an AWS RDS PostgreSQL database, and expose it via Nginx with a Cloudflare-managed domain and HTTPS.

## Prerequisites

- Ubuntu EC2 instance with Docker and Nginx installed
- AWS RDS PostgreSQL instance (with PostGIS extension enabled)
- Domain pointed to Cloudflare DNS
- SSH access to the EC2 instance
- Security groups configured to allow:
  - EC2 inbound: ports 80, 443 (HTTP/HTTPS from anywhere), port 22 (SSH)
  - RDS inbound: port 5432 from the EC2 instance's security group or private IP

## Architecture

```
User → Cloudflare (DNS only) → EC2 Nginx (:443, Certbot SSL) → Docker geopop-api (:8080) → RDS PostgreSQL (:5432)
```

---

## Step 1: Prepare the RDS Database

### 1.1 Enable PostGIS

Connect to your RDS instance and enable the PostGIS extension:

```bash
psql "postgres://YOUR_USER:YOUR_PASSWORD@YOUR_RDS_ENDPOINT:5432/YOUR_DB"
```

```sql
CREATE EXTENSION IF NOT EXISTS postgis;
```

### 1.2 Create the schema

Run the init SQL from the project to create all required tables and indexes:

```bash
psql "postgres://YOUR_USER:YOUR_PASSWORD@YOUR_RDS_ENDPOINT:5432/YOUR_DB" -f docker/init.sql
```

### 1.3 Ingest data

From a machine that can reach the RDS instance (your local machine or the EC2 instance), run the ingestion scripts pointing at the RDS database:

```bash
export DATABASE_URL="postgres://YOUR_USER:YOUR_PASSWORD@YOUR_RDS_ENDPOINT:5432/YOUR_DB"

# Install Python dependencies
pip install -r ingestion/requirements.txt

# Download datasets
make download-all

# Ingest (order matters)
python ingestion/ingest_naturalearth.py
python ingestion/ingest.py
python ingestion/ingest_geonames.py
```

This takes 30-45 minutes for the full WorldPop dataset.

---

## Step 2: Deploy the API on EC2

### 2.1 Clone the repository

```bash
ssh ubuntu@YOUR_EC2_IP
cd /opt
sudo git clone https://github.com/YOUR_USERNAME/geopop.git
sudo chown -R ubuntu:ubuntu geopop
cd geopop
```

### 2.2 Create the production `.env` file

```bash
cat > .env << 'EOF'
DATABASE_URL=postgres://YOUR_USER:YOUR_PASSWORD@YOUR_RDS_ENDPOINT:5432/YOUR_DB
API_HOST=0.0.0.0
API_PORT=8080
POOL_SIZE=16
EOF
```

Replace `YOUR_USER`, `YOUR_PASSWORD`, `YOUR_RDS_ENDPOINT`, and `YOUR_DB` with your actual RDS credentials.

### 2.3 Create a production Docker Compose file

Since you are using an external RDS database, you only need the API container. Create `docker-compose.prod.yml`:

```bash
cat > docker-compose.prod.yml << 'EOF'
services:
  api:
    build:
      context: ./api
      dockerfile: Dockerfile
    container_name: geopop-api
    env_file: .env
    ports:
      - "127.0.0.1:8080:8080"
    restart: unless-stopped
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
EOF
```

Note: `127.0.0.1:8080:8080` binds the container only to localhost so it is only accessible through Nginx, not directly from the internet.

### 2.4 Build and start

```bash
docker compose -f docker-compose.prod.yml up -d --build
```

### 2.5 Verify the API is running

```bash
docker ps
curl http://127.0.0.1:8080/api/v1/health
```

You should see `{"code":200,"message":"success","payload":{"status":"ok"}}`.

---

## Step 3: Configure Nginx as Reverse Proxy

### 3.1 Create the Nginx site configuration

```bash
sudo nano /etc/nginx/sites-available/geopop
```

Paste the following (replace `api.yourdomain.com` with your actual domain):

```nginx
server {
    listen 80;
    server_name api.yourdomain.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Timeouts for long-running analyse queries
        proxy_read_timeout 30s;
        proxy_connect_timeout 5s;
    }
}
```

### 3.2 Enable the site and restart Nginx

```bash
sudo ln -sf /etc/nginx/sites-available/geopop /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl reload nginx
```

### 3.3 Verify

```bash
curl http://api.yourdomain.com/api/v1/health
```

---

## Step 4: Configure Cloudflare DNS and SSL with Certbot

Since Cloudflare is used as DNS only (proxy disabled / grey cloud), SSL is terminated directly on the EC2 instance using a free Let's Encrypt certificate managed by Certbot. Certbot handles automatic renewal every 90 days.

### 4.1 Add DNS record in Cloudflare

Go to your Cloudflare dashboard for your domain:

1. Navigate to **DNS** > **Records**
2. Add a new record:
   - Type: `A`
   - Name: `api` (or your preferred subdomain)
   - Content: `YOUR_EC2_PUBLIC_IP`
   - Proxy status: **DNS only** (grey cloud)
   - TTL: Auto

The grey cloud (DNS only) means Cloudflare only resolves DNS — all traffic goes directly to your EC2 instance. SSL is handled by Certbot on the server itself.

### 4.2 Install Certbot

```bash
sudo apt update
sudo apt install -y certbot python3-certbot-nginx
```

### 4.3 Obtain the SSL certificate

Make sure your Nginx config from Step 3 is active and the domain resolves to your EC2 IP, then run:

```bash
sudo certbot --nginx -d api.yourdomain.com
```

Certbot will:
1. Verify domain ownership via HTTP challenge (port 80 must be open)
2. Obtain a Let's Encrypt certificate
3. Automatically modify your Nginx config to add SSL on port 443
4. Add an HTTP-to-HTTPS redirect

When prompted:
- Enter your email address (for renewal notifications)
- Agree to the terms of service
- Choose to redirect HTTP to HTTPS (recommended)

### 4.4 Verify the Nginx config

After Certbot finishes, your Nginx config at `/etc/nginx/sites-available/geopop` will look like this (Certbot modifies it automatically):

```nginx
server {
    server_name api.yourdomain.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        proxy_read_timeout 30s;
        proxy_connect_timeout 5s;
    }

    listen 443 ssl; # managed by Certbot
    ssl_certificate /etc/letsencrypt/live/api.yourdomain.com/fullchain.pem; # managed by Certbot
    ssl_certificate_key /etc/letsencrypt/live/api.yourdomain.com/privkey.pem; # managed by Certbot
    include /etc/letsencrypt/options-ssl-nginx.conf; # managed by Certbot
    ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem; # managed by Certbot
}

server {
    if ($host = api.yourdomain.com) {
        return 301 https://$host$request_uri;
    } # managed by Certbot

    listen 80;
    server_name api.yourdomain.com;
    return 404; # managed by Certbot
}
```

### 4.5 Verify auto-renewal

Certbot installs a systemd timer that automatically renews certificates before they expire (every 90 days). Verify it is active:

```bash
sudo systemctl status certbot.timer
```

You should see `active (waiting)`. Test the renewal process (dry run):

```bash
sudo certbot renew --dry-run
```

If the dry run succeeds, auto-renewal is working. No cron job needed — the systemd timer handles it.

### 4.6 Verify HTTPS

```bash
curl https://api.yourdomain.com/api/v1/health
```

You can also check the certificate details:

```bash
echo | openssl s_client -connect api.yourdomain.com:443 -servername api.yourdomain.com 2>/dev/null | openssl x509 -noout -dates
```

---

## Step 5: Post-Deployment

### 5.1 Set up auto-restart on reboot

Docker containers with `restart: unless-stopped` will restart automatically if Docker starts on boot. Make sure Docker is enabled:

```bash
sudo systemctl enable docker
```

### 5.2 Update the API

When you push code changes:

```bash
cd /opt/geopop
git pull
docker compose -f docker-compose.prod.yml up -d --build
```

### 5.3 View logs

```bash
docker logs -f geopop-api
```

### 5.4 Monitor health

```bash
curl -s https://api.yourdomain.com/api/v1/health | jq .
```

---

## Security Checklist

- [ ] EC2 security group: only allow ports 80, 443, and 22
- [ ] RDS security group: only allow port 5432 from the EC2 security group
- [ ] RDS is in a private subnet (not publicly accessible)
- [ ] Strong RDS password (not the default `geopop`)
- [ ] `.env` file has `chmod 600` and is not committed to git
- [ ] Nginx binds the API to `127.0.0.1` only (not exposed directly)
- [ ] Certbot auto-renewal timer is active (`sudo systemctl status certbot.timer`)
- [ ] HTTP redirects to HTTPS (Certbot handles this automatically)

---

## Troubleshooting

**API container won't start / unhealthy:**

```bash
docker logs geopop-api
```

Common causes: RDS not reachable (check security groups), wrong `DATABASE_URL`, PostGIS not enabled.

**502 Bad Gateway from Nginx:**

The API container is not running or not listening on port 8080. Check `docker ps` and `docker logs geopop-api`.

**SSL certificate not renewing:**

Check the Certbot timer: `sudo systemctl status certbot.timer`. If inactive, enable it: `sudo systemctl enable --now certbot.timer`. You can also manually renew with `sudo certbot renew`.

**ERR_CONNECTION_REFUSED on HTTPS:**

Port 443 is not open in the EC2 security group, or Nginx is not listening on 443. Check `sudo nginx -t` and verify the security group allows inbound 443 from `0.0.0.0/0`.

**Slow `/analyse` responses for ocean coordinates:**

The auto-expanding radius search may need to expand up to 1000 km for very remote ocean coordinates. This is expected and can take 1-3 seconds. Consider adding a CDN cache rule in Cloudflare for GET requests with a short TTL if needed.

**RDS connection timeout:**

Ensure the EC2 and RDS are in the same VPC, or that VPC peering / public access is configured. Test connectivity:

```bash
nc -zv YOUR_RDS_ENDPOINT 5432
```
