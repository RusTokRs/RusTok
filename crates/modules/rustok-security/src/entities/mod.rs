/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! SeaORM persistence mappings owned by security.

pub mod audit_log;
pub mod ip_rule;
pub mod policy;

pub use audit_log::Entity as SecurityAuditLogEntity;
pub use ip_rule::Entity as SecurityIpRuleEntity;
pub use policy::Entity as SecurityPolicyEntity;
