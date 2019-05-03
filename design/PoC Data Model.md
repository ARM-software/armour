Digital Assets
==============

### Subjects

Name                | ID         | Notes
--------------------|------------|------------------------------
Service Provider    | SP         |
Health Authority    | HA         |
Pharmaceutical      | Pharm[id]  |
Insurance           | Insure[id] |
Healthcare Provider | HC[id]     | GPs and medical practitioners
Patient             | P[id]      | Individuals

### Resources and Capabilities

Subject    | Resource      | Managed by | Type (Requirements) | Capabilities
-----------|---------------|------------|---------------------|------------------------------------
SP         | name          | SP         | text                | read:\*, write:SP
           | app           | SP         | code, data, text    | read:SP, write:SP, execute:SP
           | id            | SP         | certificate         | read:\*
           | root key      | SP         | key                 | read:SP, write:SP
HA[id]     | id            | SP         | certificate         | read:\*
           | private key   | SP         | key                 | read:SP, write:SP
           | name          | HA[id]     | text                | read:\*, write:SP(oc)
           | app           | HA[id]     | code, data, text    | read:SP(oc)+HA[id], write:SP(oc)+HA[id], execute:SP
Pharm[id]  | id            | SP         | certificate         | read:\*
           | private key   | SP         | key                 | read:SP, write:SP
           | name          | Pharm[id]  | text                | read:\*, write:SP(oc)+Pharm[id]
           | app           | Pharm[id]  | code, data, text.   | read:SP(oc)+Pharm[id], write:SP(oc)+Pharm[id], execute:SP
Insure[id] | id            | SP         | certificate         | read:\*
           | private key   | SP         | key                 | read:SP, write:SP
           | name          | Insure[id] | text                | read:\*, write:SP+Insure[id]
           | app           | Insure[id] | code, data, text    | read:SP(oc)+Insure[id], write:SP(oc)+Insure[id], execute:SP
HC[id]     | id            | SP         | auth factor         | read:SP
           | name          | SP         | text                | read:SP+HA(oc)+P\[id\](oc), write:SP(oc)
P[id]      | id            | SP         | auth factor         | read:SP
           | name          | SP         | text                | read:SP+HA\[id\](oc)+HC[id]+P[id], write:SP(oc)
           | age           | SP         | text                | read:SP+HA\[id\](oc)+HC[id]+P[id]
           | address       | SP         | text                | read:SP+HA\[id\](oc)+HC[id]+P[id]
           | medical notes | SP         | text (enc+backup)   | read:HA\[id\](oc+anon)+HC[id]+P[id]
           | medical data  | SP         | data (enc+backup)   | read:HA\[id\](oc+anon)+HC[id]+P[id]
           | vitals        | SP         | data                | read:HA\[id\](oc+anon)+HC[id]+P[id]

### Key

- (\*) any
- (oc) owner/subject consent
- (c X) consent from X
- (anon) anonymised
- (enc) encrypted