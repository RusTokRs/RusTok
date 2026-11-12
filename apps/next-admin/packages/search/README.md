# @rustok/search-admin

Module-owned Next.js administration package for the RusToK Search platform.

## Overview

This package provides the administration workspace for search indexing, engine switching, diagnostic metrics, and search intelligence:

- **Overview**: Real-time diagnostic statistics, index lag tracking, consistency issues, and full/selective index rebuild triggers.
- **Playground**: Live preview of search results across active search engines, scoring breakdown, facet aggregations, and ranking profile experiments.
- **Analytics**: Search query volume, zero-result queries, slow searches, click-through rates (CTR), abandonment analysis, and intelligence candidates.
- **Dictionaries**: Search synonyms, stopwords, and pinned document query rules (curation rules).

## Architecture

- **Host Integration**: Registers `/dashboard/search` under the `modulePlugins` navigation group via `@/modules/registry`.
- **Backend Boundary**: Communicates with the async-graphql search API via `graphqlRequest` with tenant and session token propagation.
- **Shared FFA Primitives**: Renders data using `@/widgets/data-table` and shared UI components.
