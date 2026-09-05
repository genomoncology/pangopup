---
base: db0fe7f2fa8da13732cc0e2edbdcfbfd97fe2a46
head: 838ea999e31bdce4bdb2783554ba68b7b66f4899
---

# Answer rejected model input with a client status

The HTTP scoring route now returns 422 when the backend reports `MODEL_REJECTED`. Backend scoring and cache failures remain 500. The response keeps the existing error code, generic message, and JSON shape.

Independent design and code reviews accepted the change without findings. Service tests pin every backend failure family. A real executable and miniature profile prove the public rejection path. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 277 passed and 7 skipped.
