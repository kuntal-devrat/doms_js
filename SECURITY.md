# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the current minor release branch and the active development branch.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1.0 | :x:                |

## Reporting a Vulnerability

The DOMS team takes security vulnerabilities seriously. Because DOMS is designed to evaluate JavaScript code and support multi-isolate parallel execution with sandboxing and memory isolation primitives, security bugs (such as sandbox escapes, use-after-free in the GC, interpreter type confusions, or FFI memory corruption) are treated with top priority.

If you believe you have discovered a vulnerability in DOMS:

1. **Do not open a public GitHub issue.**
2. Send an email to **security@domsjs.dev** (or open a private GitHub Security Advisory under the repository's `Security` tab).
3. Include detailed reproduction steps, the environment/platform (OS, architecture), sample JavaScript code that triggers the issue, and expected vs observed behavior.

### What to Expect

- **Acknowledgment:** We will acknowledge receipt of your report within 48 hours.
- **Assessment:** We will confirm the vulnerability and determine its impact and severity within 5 business days.
- **Remediation:** We will work on a patch and coordinate a disclosure timeline (typically 90 days or faster).
- **Credit:** We will publicly credit reporters in the release notes and CHANGELOG (unless anonymity is requested).
