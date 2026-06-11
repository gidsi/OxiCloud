# 🏅 Priority 4: Multiple Calendars & Address Books

**As a** busy professional,  
**I want to** create and manage multiple separate calendars (e.g., Work, Personal, Family) and address books,  
**so that** I can keep my schedules and contacts organised and toggle their visibility separately in my client.

**Context for the Squad:** 
Now that the engine works, we need to let users organise their lives. If a client sends a command to create a new calendar collection, the server must support it via PostgreSQL structural boundaries, strictly enforcing user data isolation in `sqlx`.

**Acceptance Criteria:**
