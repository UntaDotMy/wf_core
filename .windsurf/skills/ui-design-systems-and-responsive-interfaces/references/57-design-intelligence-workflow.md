# Design Intelligence Workflow

Use this reference when you need a structured UI recommendation packet from local evidence rather than freeform invention. This wf-core package does not ship a `design-intelligence` CLI command; build the packet manually from the skill references and repository evidence.

## Backing Catalog

`data/design_intelligence_catalog.json`

## Recommended Use

1. Start with the raw product or feature query and turn it into a working brief.
2. Read the current repository UI, components, tokens, content, states, and breakpoints.
3. Compare two or three mature product-family benchmarks and extract design principles without copying them.
4. Produce a first-pass design intelligence packet: visual direction, interaction model, responsive rules, accessibility checks, and recovery states.
5. Add stack and component-library constraints when implementation realities should shape the recommendation.
6. Persist design-system notes only if shared documentation will help the team.
7. Validate components and states in isolated tooling or the running app when available.

## Output Shape Highlights

A useful packet includes:

- stack-aware adaptation guidance when framework or component-library constraints exist
- professional polish checks for affordance, CTA clarity, contrast, density, and layout stability
- recovery checks for validation, interruption, empty/error/loading states, and high-trust flows
- product-family-aware recommendations for familiar surfaces such as messaging, checkout, dashboards, or onboarding
- explicit clarification questions when the prompt is too vague to classify safely

## Persistence Safety

If writing design-system docs:

- normalize project and page names to safe slugs
- create parent directories before writing
- use a master document as the source of truth and page files as overrides
- keep generated docs reviewable and reversible
