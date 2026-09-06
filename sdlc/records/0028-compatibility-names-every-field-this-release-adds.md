---
base: 3d89aca
head: 090b1c822bdf8efeca4dd8bf0d74fbaed26060f2
---

# Compatibility names every response-shape addition in 0.4

The durable compatibility contract and v0.4.0 candidate release notes now inventory every response-shape change from v0.3.0. They cover status identity and request contract fields, model status fields, common score-item fields, the new rejected status and both rejected-item shapes, rejected-item error fields, and the stable gene field. They state one consumer-first deployment order for strict JSON readers.

The HTTP specification now publishes the complete nested `request_contract` schema. The normal version-consistency gate protects both compatibility documents, every inventory entry and object scope, both rejected-item shapes, both schema links, the deployment order, and all 25 nested schema members. The Python 3.9 mutation test holds an independent expected inventory and proves those checks fail when required material disappears or changes scope.

Design review expanded the original five-field draft to cover the complete v0.3-to-v0.4 response difference. Code review found the missing rejected-item shapes, an unprotected nested schema, a self-referential mutation test, and one inaccurate framing sentence. Remediation resolved every finding, and independent re-review accepted the result. `make lint`, the focused HTTP specification, the Python 3.9 mutation suite, formatting, and `git diff --check` passed before the implementation commit.
