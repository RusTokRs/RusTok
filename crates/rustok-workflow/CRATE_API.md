# rustok-workflow — Public API

## Module registration

```rust
use rustok_workflow::WorkflowModule;

// In apps/server module registry:
registry.register(Box::new(WorkflowModule));
```

`WorkflowModule` implements `RusToKModule`:
- `slug()` → `"workflow"`
- `name()` → `"Workflow"`
- `kind()` → `ModuleKind::Optional`
- `dependencies()` → `["core"]`
- `migrations()` → `WorkflowsMigration`, `WorkflowPhase4Migration`
- `permissions()` → `Workflows::{Create,Read,Update,Delete,List,Execute,Manage}`, `WorkflowExecutions::{Read,List}`

## Services

### `WorkflowService`

```rust
pub struct WorkflowService { /* db: DatabaseConnection */ }

impl WorkflowService {
    pub fn new(db: DatabaseConnection) -> Self;

    // Workflows
    pub async fn create_workflow(&self, tenant_id: Uuid, dto: CreateWorkflowDto) -> WorkflowResult<WorkflowDto>;
    pub async fn get_workflow(&self, id: Uuid, tenant_id: Uuid) -> WorkflowResult<WorkflowDto>;
    pub async fn list_workflows(&self, tenant_id: Uuid) -> WorkflowResult<Vec<WorkflowDto>>;
    pub async fn update_workflow(&self, id: Uuid, tenant_id: Uuid, dto: UpdateWorkflowDto) -> WorkflowResult<WorkflowDto>;
    pub async fn delete_workflow(&self, id: Uuid, tenant_id: Uuid) -> WorkflowResult<()>;
    pub async fn activate_workflow(&self, id: Uuid, tenant_id: Uuid) -> WorkflowResult<WorkflowDto>;

    // Steps
    pub async fn add_step(&self, workflow_id: Uuid, tenant_id: Uuid, dto: CreateStepDto) -> WorkflowResult<WorkflowStepDto>;
    pub async fn update_step(&self, step_id: Uuid, tenant_id: Uuid, dto: UpdateStepDto) -> WorkflowResult<WorkflowStepDto>;
    pub async fn delete_step(&self, step_id: Uuid, tenant_id: Uuid) -> WorkflowResult<()>;

    // Executions
    pub async fn list_executions(&self, workflow_id: Uuid, tenant_id: Uuid) -> WorkflowResult<Vec<WorkflowExecutionDto>>;
    pub async fn get_execution(&self, execution_id: Uuid, tenant_id: Uuid) -> WorkflowResult<WorkflowExecutionDto>;
}
```

### `WorkflowEngine`

```rust
pub struct WorkflowEngine { /* db, step registry */ }

impl WorkflowEngine {
    pub fn new(db: DatabaseConnection) -> Self;
    pub fn with_step(mut self, step_type: impl Into<String>, step: Arc<dyn WorkflowStep>) -> Self;
    pub async fn execute(&self, workflow_id: Uuid, trigger_event_id: Uuid, context: Value) -> WorkflowResult<Uuid>;
}
```

### `WorkflowTriggerHandler`

```rust
pub struct WorkflowTriggerHandler { /* db, engine */ }

impl WorkflowTriggerHandler {
    pub fn new(db: DatabaseConnection, engine: Arc<WorkflowEngine>) -> Self;
    // Implements EventHandler — call from EventBus subscriber loop
    pub async fn handle(&self, event: &EventEnvelope) -> WorkflowResult<()>;
}
```

### `WorkflowCronScheduler`

```rust
pub struct WorkflowCronScheduler { /* db, engine */ }

impl WorkflowCronScheduler {
    pub fn new(db: DatabaseConnection, engine: Arc<WorkflowEngine>) -> Self;
    pub async fn tick(&self) -> WorkflowResult<()>;
}
```

## Step extension trait

```rust
#[async_trait]
pub trait WorkflowStep: Send + Sync {
    async fn execute(&self, config: &Value, ctx: &mut StepContext) -> WorkflowResult<()>;
}
```

Modules can register custom step types via `WorkflowEngine::with_step(...)`.

## Templates

```rust
pub static BUILTIN_TEMPLATES: &[WorkflowTemplate];

pub struct WorkflowTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    // ...
}
```

## Error type

```rust
pub enum WorkflowError {
    NotFound(Uuid),
    Unauthorized,
    InvalidConfig(String),
    StepFailed { step_id: Uuid, reason: String },
    Database(DbErr),
}

pub type WorkflowResult<T> = Result<T, WorkflowError>;
```

## DTOs

Key request/response types (all in `rustok_workflow::dto`):

- `CreateWorkflowDto` / `UpdateWorkflowDto` / `WorkflowDto`
- `CreateStepDto` / `UpdateStepDto` / `WorkflowStepDto`
- `WorkflowExecutionDto` / `WorkflowStepExecutionDto`
