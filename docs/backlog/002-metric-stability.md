# Priority 2: Metric Stability (Cardinality & Memory Safeguards)

**Context:** We cannot ship an observability feature that doubles as a Denial of Service (DoS) vulnerability. Self-hosters expose their instances to the open internet, meaning bots will scan random, non-existent URLs. If we dynamically create a metric label for every URL requested, OxiCloud will consume all server memory and crash, interrupting file and calendar sync.

**User Story:**
As a Server Admin, I want OxiCloud's metrics collection to enforce strict static labels and memory limits so that my server stays online and doesn't crash from Out-Of-Memory (OOM) errors when bots or scanners spam random URLs.

**Acceptance Criteria:**
