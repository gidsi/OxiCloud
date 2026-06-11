# 🥇 Priority 1: Basic Calendar Sync (CalDAV MVP)

**As a** privacy-conscious self-hoster,  
**I want to** connect my calendar clients (Thunderbird, DAVx5, GNOME Calendar) to my OxiCloud account to sync my events,  
**so that** I can manage my schedule securely across all my devices without relying on Big Tech.

**Context for the Squad:** 
This is the core engine. We need to handle basic authentication, calendar discovery, and full CRUD (Create, Read, Update, Delete) of `.ics` events in a single default calendar per user. Do not split this up. A synced calendar must be fully functional.

**Acceptance Criteria:**
