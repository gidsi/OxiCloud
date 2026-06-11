# 🥉 Priority 3: Frictionless Client Auto-Discovery 

**As a** non-technical user or busy self-hoster,  
**I want to** simply enter my server domain and login credentials into Apple Calendar or DAVx5,  
**so that** my calendars and contacts configure themselves without me having to type out massive, obscure server URLs.

**Context for the Squad:** 
Apple Calendar is notoriously stubborn. If we don't implement `.well-known` URIs, users will think our server is broken because they won't know they have to manually type full explicit paths. This routes Axum to handle `/.well-known/` gracefully and bridges the gap between "technically works" and "actually usable."

**Acceptance Criteria:**
