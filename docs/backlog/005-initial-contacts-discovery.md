### 5. Initial Contacts Discovery (CardDAV Read-only Sync)
*Priority: 5 - Shifting to Contacts. Viewing contacts on a phone is the critical first step to replacing Google Contacts.*

**As a** privacy-conscious smartphone user, **I want to** connect my phone's contact app to OxiCloud via DAVx5 **so that** I can access my securely hosted address book to identify incoming callers.

**Acceptance Criteria:**
*   **Given** an OxiCloud account populated with existing contacts (names and phone numbers)
*   **When** the user adds an OxiCloud CardDAV account in DAVx5 on their Android device
*   **Then** the contacts are successfully downloaded to the device
*   **And** the contacts populate accurately in the native Android Dialer and Contacts applications.

**Security Constraints:**
*   **Data Masking/Logging:** Contacts contain highly sensitive PII. Ensure our Tokio tracing/logging layers absolutely *never* log the content of vCard payloads or request bodies.
*   **Address Book Ownership:** Strict authorization middleware verifying the authenticated user against the address book ID.

**Architectural Constraints:**
*   **CardDAV Protocol (RFC 6352):** Implement `.well-known/carddav` redirect. Address books live at `/dav/addressbooks/{user}/{book_id}/`.
*   **Code Reusability:** The WebDAV foundation (Basic Auth, XML generation, PROPFIND handling) built for CalDAV must be abstracted in the Application Layer to be reused for CardDAV. Only the domain entities and specific XML namespaces should differ.

**Tech Lead Synthesis & Risks:**
*   **Risk - vCard Formats:** Clients use varying versions of vCard (v3.0 vs v4.0). We must store the raw `.vcf` payload as an opaque blob in PostgreSQL (`TEXT`), extracting only the UID for the database index. Reconstructing vCards server-side is a well-known anti-pattern that leads to data corruption for obscure client fields.
