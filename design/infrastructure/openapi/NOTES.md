# OpenAPI — Design Notes

## Tag API Presentation (2026-04-15)

Tags are stored as `(category, name)` in the domain. The API presents them in two formats depending on context:

- **Dictionary format** for full listings (responses): `{ "organization": ["StudioFox"], "metadata": ["ref_sheet"] }`
- **`TYPE:tag` string format** for inline references (query params, filters): `general:wolf,metadata:ref_sheet`

This is purely API presentation — no domain or storage changes. Both formats parse to `(TagCategory, name)` internally.
