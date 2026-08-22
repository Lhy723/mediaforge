# Security Policy

## Supported versions

Only the latest version on the `main` branch is currently supported with
security fixes.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Contact the
maintainer privately through the security contact configured on the GitHub
repository, or use GitHub's private vulnerability reporting flow when it is
available.

Include a clear description, reproduction steps, affected versions, and any
mitigation you have identified. We will acknowledge a report as soon as
practical and keep the reporter informed while it is investigated.

MediaForge invokes local FFmpeg/FFprobe processes. Treat input media and Raw
FFmpeg arguments as untrusted when they originate outside your own agent or
filesystem, and run the tool with the minimum operating-system permissions
needed for the job.
