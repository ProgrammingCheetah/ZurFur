# OpenAPI Integration — Design Questions

Answer inline after each question. When done, tell me to check this file.

---

## Crate & UI

**Q1: UI preference?**
The main options for interactive API docs:
- **Swagger UI** — industry standard, familiar to most developers
- **Scalar** — modern, cleaner design, better DX
- **RapiDoc** — lightweight alternative
- **Multiple** — we can serve more than one

Which do you prefer?
> Let's go with Scalar. I feel like we need to be modern and accept new technologies.
**Q2: Spec endpoint path?**
Where should the OpenAPI JSON spec be served? Common choices:
- `/api-docs/openapi.json` (spec) + `/api-docs` or `/swagger-ui` (UI)
- `/docs` (UI) + `/docs/openapi.json` (spec)

Any preference?
> I think we should do `/api/docs` for the UI and `/api/docs/openapi.json` for the spec

**Q3: Environment gating?**
Should the docs UI be available in production, or dev/staging only? The JSON spec could still be available in prod for client generation even if the UI is hidden.
> The idea is for everything that we are making to be able to be used everywhere, as per decentralization. I think we should strive for at least most of it to be public.
## Scope

**Q4: Annotate everything now?**
Should I annotate all 35 existing endpoints in this pass, or set up the infrastructure + annotate a subset (e.g., one module) as a pattern, then do the rest incrementally?
> Yes! We should make a branch for this under the OpenAPI addition. We should be able to document as much of the API as we possibly can.

**Q5: Static spec file?**
Should we also generate a static `openapi.json` file in the repo (for external tools, CI validation, client codegen)? Or is the runtime endpoint sufficient for now?
> I believe this is a good idea, yes. We need to be as transparent as possible. However, we should make stuff as automatic as possible, through scripts or the like.

## Schema & Naming

**Q6: API grouping?**
utoipa groups endpoints into tags (collapsible sections in the UI). Natural grouping would mirror the route modules:
- Auth, Users, Organizations, Feeds, Tags, Onboarding

Any different grouping preference?
> Nope! This sounds like it even follows OUR architecture regarding tags

**Q7: Response envelope?**
Currently, success responses return the data directly (e.g., `{ "id": "...", "name": "..." }`) and errors use `{ "error": "...", "code": "..." }`. Should we document this as-is, or is this a chance to standardize something different?
> We should standardize!

## Testing

**Q8: What kind of testing did you have in mind?**
"OpenAPI for testing" could mean several things:
- **Interactive testing** via Swagger UI (manual, click-and-send)
- **Contract testing** (validate API responses match the spec in CI)
- **Client generation** (generate a TypeScript/etc. client from the spec for frontend)
- **Request validation middleware** (reject requests that don't match the spec)

Which of these are you after?

> We can work on this later, but I meant Contract testing + Client Generation
