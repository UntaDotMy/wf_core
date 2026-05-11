---
description: "Trace behavior ownership before changing existing code"
trigger: always_on
---

# Preserve Existing Flow

Before patching existing behavior, identify the entry point, producer, source of
truth, state/storage/queue owner, side-effect owner, consumers, cleanup or
recovery path, edit boundary, validation needed, and validation evidence.

Do not patch the first suspicious branch until the owner path is understood.
Greenfield, docs-only, formatting-only, and generated-only changes can be exempt
when explicitly labeled.
