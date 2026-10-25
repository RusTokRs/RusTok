/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Application and domain services orchestrating business use cases for Calendar & Appointment Scheduling.

use crate::error::CalendarResult;

#[derive(Clone, Debug, Default)]
pub struct CalendarService;

impl CalendarService {
    pub fn new() -> Self {
        Self
    }

    pub fn ping(&self) -> CalendarResult<&'static str> {
        Ok("pong")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_ping() {
        let service = CalendarService::new();
        assert_eq!(service.ping().unwrap(), "pong");
    }
}
