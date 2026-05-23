# Priority 1: Deep Health Check for Uptime Monitoring

**Context:** Self-hosters use tools like Uptime Kuma, Docker auto-heal, or reverse proxies. A simple "200 OK" from the web server isn't enough; if the PostgreSQL database or the filesystem drops, users can't sync their calendars or files. We need the health check to verify the actual underlying infrastructure.

**User Story:**
As a Server Admin, I want to query a deep health check at `/health` so that my monitoring tools know exactly when my database or file storage goes offline and can alert me before my users experience sync failures.

**Acceptance Criteria:**
