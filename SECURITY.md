# Security Policy for pgQuant

## Supported Versions

`pgQuant` is an actively maintained PostgreSQL extension. We provide security updates primarily for the latest stable release of the extension across all supported PostgreSQL versions (14, 15, 16, 17).

| pgQuant Version | Supported          | Notes                                      |
| --------------- | ------------------ | ------------------------------------------ |
| Latest Release  | :white_check_mark: | Active support and security updates        |
| Older Releases  | :x:                | Please upgrade to the latest version       |

*Note: Since `pgQuant` processes data directly within PostgreSQL, ensure your underlying PostgreSQL installation and OS are also kept up-to-date with security patches.*

## Reporting a Vulnerability

If you discover a potential vulnerability in `pgQuant` (e.g., memory unsafety in the Rust extensions, SQL injection vectors in the query parsers, or any data leakage), please do NOT open a public issue. 

Instead, report it via email to the maintainer or by creating a private security advisory on this GitHub repository. 

We will endeavor to respond to your report within 48 hours. If the vulnerability is accepted, we will work with you to understand the issue, provide a timeline for a fix, and coordinate a public release and disclosure.
