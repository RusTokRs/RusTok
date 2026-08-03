#!/usr/bin/env node

// Compatibility entrypoint retained for existing maintainer commands.
// Product Admin primary GraphQL read error-safety verification failed:
// expected five primary read mappers plus the retained catalog-options mapper
// Product Admin primary GraphQL reads retain typed errors with correlation-safe static public payloads; execution evidence remains open
import "./verify-product-admin-graphql-read-diagnostic-safety.mjs";
