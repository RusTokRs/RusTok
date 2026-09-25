#!/usr/bin/env node

import { readFileSync } from 'node:fs';
const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const failures = [];
const requireText = (s,v,l)=>{if(!s.includes(v))failures.push(`${l}: missing ${v}`);};
const forbidText = (s,v,l)=>{if(s.includes(v))failures.push(`${l}: forbidden ${v}`);};
const source=read('crates/modules/rustok-commerce/src/services/checkout_plan_builder.rs');
for(const marker of ['Arc<dyn ShippingOptionReadPort>','in_process_shipping_option_read_port(','ReadShippingOptionProjectionRequest {','.read_shipping_option_projection(','with_shipping_option_read_port(','checkout_plan_shipping_option_boundary_error(')]) requireText(source,marker,'checkout plan shipping-option owner boundary');
forbidText(source,'FulfillmentService::new(','checkout plan must not construct FulfillmentService directly');
forbidText(source,'self.fulfillment_service','checkout plan must not retain a concrete fulfillment service field');
if(failures.length){for(const failure of failures) console.error(failure);process.exit(1);}
console.log('checkout plan shipping-option owner boundary: ok');
