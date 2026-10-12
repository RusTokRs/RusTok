# rustok-docs Documentation

This directory contains architectural and implementation documentation for `rustok-docs`.

## Architecture Overview

`rustok-docs` implements a Notion-grade collaborative document, wiki, and structured database engine natively in Rust:
- **Loro CRDT:** High-performance CRDT engine compiling natively into Rust, providing deterministic concurrent editing and block reordering without split-brain documents.
- **Universal Entity Reference Protocol (UER):** Uniform cross-domain embedding protocol linking collaborative documents with Media, Blog Posts, Forum Topics, Wiki Pages, Products, CRM Deals, and Billing Invoices.
- **Bidirectional Backlinks & Knowledge Graph:** Transactional index `docs_entity_links` powering reverse backlink panels in consumer modules and LLM reasoning in `rustok-mcp`.

## Master Plan

Refer to the canonical platform roadmap:
- [Workplace & Collaboration SuperApp Plan](../../../../docs/architecture/workplace-collaboration-superapp-plan.md)
