# Priority 3: Prometheus Metrics Endpoint

**Context:** Once the server is reporting health and our metric collection is safe from memory exhaustion, we can expose the endpoint. This allows advanced home-labbers and admins to pipe OxiCloud data into Grafana to identify sync bottlenecks.

**User Story:**
As a Server Admin, I want to scrape OxiCloud's performance data at `/metrics` so that I can visualize server load, API error rates, and database connection pool usage in my Grafana dashboard.

**Acceptance Criteria:**
