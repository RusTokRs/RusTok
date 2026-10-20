#!/usr/bin/env node
// Superseded transport preflight/mapper checks: Product now owns receipt-aware admission.
// Retained CLI entrypoint checks the replacement owner/provider/transport contract.
import {run} from './verify-product-shipping-admission.mjs';
run();
