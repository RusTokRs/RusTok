# Migration Guide

Полное руководство по миграции с других revision history решений на rustok-revisions.

## Содержание

1. [Миграция с PaperTrail (Ruby)](#миграция-с-papertrail-ruby)
2. [Миграция с Django Simple History](#миграция-с-django-simple-history)
3. [Миграция с Django Reversion](#миграция-с-django-reversion)
4. [Миграция с Git-based решений](#миграция-с-git-based-решений)
5. [Миграция с Custom решений](#миграция-с-custom-решений)
6. [Общие принципы миграции](#общие-принципы-миграции)
7. [Data Migration Scripts](#data-migration-scripts)
8. [Verification](#verification)

## Миграция с PaperTrail (Ruby)

### Обзор PaperTrail

**PaperTrail** — популярная Ruby библиотека для отслеживания изменений моделей ActiveRecord.

**Особенности:**
- Full snapshot storage (хранит полные копии объектов)
- Таблица `versions` с полями: `item_type`, `item_id`, `event`, `object`, `created_at`
- Автоматическое отслеживание через callbacks
- Поддержка `has_paper_trail` macro

### Сравнение с rustok-revisions

| Feature | PaperTrail | rustok-revisions |
|---------|-----------|------------------|
| Storage | Full snapshots | Delta-based |
| Language | Ruby | Rust |
| Framework | Rails | Any Rust framework |
| Database | Any (ActiveRecord) | PostgreSQL (SeaORM) |
| Async | No | Yes (tokio) |
| Multilingual | No | Yes |
| Named versions | No | Yes |

### Шаги миграции

#### 1. Анализ существующей структуры

```ruby
# PaperTrail schema
create_table :versions do |t|
  t.string   :item_type, null: false
  t.integer  :item_id,   null: false
  t.string   :event,     null: false  # create, update, destroy
  t.text     :object                  # YAML serialized object
  t.text     :object_changes          # YAML serialized changes
  t.datetime :created_at
end

add_index :versions, [:item_type, :item_id]
```

#### 2. Создание новой структуры

```rust
// rustok-revisions schema
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_revisions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub tenant_id: String,
    pub content_type: String,
    pub content_id: String,
    pub locale: Option<String>,
    pub revision_number: i32,
    pub delta: Json,  // JSON вместо YAML
    pub created_by: Option<String>,
    pub created_at: DateTimeUtc,
    pub change_source: Option<String>,
    pub change_summary: Option<String>,
    pub version_name: Option<String>,
}
```

#### 3. Экспорт данных из PaperTrail

```ruby
# export_paper_trail.rb
require 'csv'
require 'yaml'

CSV.open('paper_trail_export.csv', 'w') do |csv|
  csv << ['item_type', 'item_id', 'event', 'object', 'created_at', 'whodunnit']
  
  Version.find_each do |version|
    csv << [
      version.item_type,
      version.item_id,
      version.event,
      version.object,  # YAML
      version.created_at.iso8601,
      version.whodunnit
    ]
  end
end
```

#### 4. Трансформация данных

```python
# transform_paper_trail.py
import csv
import yaml
import json

def transform_paper_trail_to_rustok(input_file, output_file):
    with open(input_file, 'r') as infile, open(output_file, 'w') as outfile:
        reader = csv.DictReader(infile)
        writer = csv.DictWriter(outfile, fieldnames=[
            'content_type', 'content_id', 'delta', 'created_at', 'created_by'
        ])
        writer.writeheader()
        
        for row in reader:
            # Парсим YAML object
            object_data = yaml.safe_load(row['object'])
            
            # Конвертируем в delta format
            if row['event'] == 'create':
                delta = {k: {'old': None, 'new': v} for k, v in object_data.items()}
            elif row['event'] == 'update':
                # Для update нужен object_changes
                changes = yaml.safe_load(row.get('object_changes', '{}'))
                delta = {}
                for field, (old_val, new_val) in changes.items():
                    delta[field] = {'old': old_val, 'new': new_val}
            elif row['event'] == 'destroy':
                delta = {k: {'old': v, 'new': None} for k, v in object_data.items()}
            
            # Маппинг типов
            content_type = map_content_type(row['item_type'])
            
            writer.writerow({
                'content_type': content_type,
                'content_id': f"{row['item_type'].lower()}-{row['item_id']}",
                'delta': json.dumps(delta),
                'created_at': row['created_at'],
                'created_by': row['whodunnit'] or 'system'
            })

def map_content_type(item_type):
    mapping = {
        'Post': 'blog_post',
        'Comment': 'blog_comment',
        'User': 'user',
        'Product': 'product',
    }
    return mapping.get(item_type, item_type.lower())

if __name__ == '__main__':
    transform_paper_trail_to_rustok(
        'paper_trail_export.csv',
        'rustok_import.csv'
    )
```

#### 5. Импорт в rustok-revisions

```rust
// import_from_paper_trail.rs
use csv::ReaderBuilder;
use serde::Deserialize;
use rustok_revisions::{RevisionService, SeaORMBackend};

#[derive(Debug, Deserialize)]
struct PaperTrailRecord {
    content_type: String,
    content_id: String,
    delta: String,
    created_at: String,
    created_by: String,
}

async fn import_from_paper_trail(
    service: &RevisionService,
    csv_file: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut reader = ReaderBuilder::new().from_path(csv_file)?;
    let mut imported = 0;
    
    for result in reader.deserialize() {
        let record: PaperTrailRecord = result?;
        
        let delta: serde_json::Value = serde_json::from_str(&record.delta)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&record.created_at)?;
        
        // Создаем revision
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id: "default".to_string(),
            content_type: record.content_type,
            content_id: record.content_id,
            locale: None,
            revision_number: 0,  // Будет пересчитан
            parent_revision_id: None,
            delta,
            created_by: Some(record.created_by),
            created_at: created_at.with_timezone(&Utc),
            change_source: Some("paper_trail_migration".to_string()),
            change_summary: Some("Imported from PaperTrail".to_string()),
            version_name: None,
        };
        
        service.backend().create_revision(revision).await?;
        imported += 1;
        
        if imported % 1000 == 0 {
            println!("Imported {} records", imported);
        }
    }
    
    Ok(imported)
}
```

#### 6. Верификация

```sql
-- Проверяем количество записей
SELECT count(*) FROM content_revisions;

-- Проверяем распределение по типам
SELECT content_type, count(*) 
FROM content_revisions 
GROUP BY content_type 
ORDER BY count(*) DESC;

-- Проверяем даты
SELECT 
    min(created_at) as earliest,
    max(created_at) as latest
FROM content_revisions;
```

## Миграция с Django Simple History

### Обзор Django Simple History

**Django Simple History** — популярная Python библиотека для отслеживания изменений Django моделей.

**Особенности:**
- Создает отдельную таблицу `historical_<model>` для каждой модели
- Full snapshot storage
- Автоматическое отслеживание через signals
- Поддержка `as_of()` для point-in-time queries

### Сравнение с rustok-revisions

| Feature | Django Simple History | rustok-revisions |
|---------|----------------------|------------------|
| Storage | Full snapshots | Delta-based |
| Language | Python | Rust |
| Framework | Django | Any Rust framework |
| Tables | Separate per model | Single table |
| Async | No | Yes (tokio) |
| Point-in-time | Yes (`as_of`) | Yes (`get_revision_at_time`) |

### Шаги миграции

#### 1. Анализ существующей структуры

```python
# Django Simple History schema
class HistoricalPost(models.Model):
    id = models.IntegerField()
    title = models.CharField(max_length=200)
    content = models.TextField()
    history_id = models.AutoField(primary_key=True)
    history_date = models.DateTimeField()
    history_change_reason = models.CharField(max_length=100, null=True)
    history_type = models.CharField(max_length=1)  # +, ~, -
    history_user = models.ForeignKey(User, null=True)
```

#### 2. Экспорт данных

```python
# export_django_history.py
from django.core.management.base import BaseCommand
from myapp.models import HistoricalPost
import csv
import json

class Command(BaseCommand):
    def handle(self, *args, **options):
        with open('django_history_export.csv', 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow([
                'model', 'object_id', 'history_type', 
                'history_date', 'history_user', 'data'
            ])
            
            for hist in HistoricalPost.objects.all().order_by('history_date'):
                data = {
                    'title': hist.title,
                    'content': hist.content,
                    # ... другие поля
                }
                
                writer.writerow([
                    'Post',
                    hist.id,
                    hist.history_type,
                    hist.history_date.isoformat(),
                    hist.history_user_id or 'system',
                    json.dumps(data)
                ])
```

#### 3. Трансформация данных

```python
# transform_django_history.py
import csv
import json
from collections import defaultdict

def transform_django_to_rustok(input_file, output_file):
    # Группируем по объекту для вычисления deltas
    objects = defaultdict(list)
    
    with open(input_file, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = f"{row['model']}-{row['object_id']}"
            objects[key].append(row)
    
    # Сортируем по дате
    for key in objects:
        objects[key].sort(key=lambda x: x['history_date'])
    
    # Вычисляем deltas
    with open(output_file, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=[
            'content_type', 'content_id', 'delta', 'created_at', 'created_by'
        ])
        writer.writeheader()
        
        for key, revisions in objects.items():
            prev_data = None
            
            for revision in revisions:
                current_data = json.loads(revision['data'])
                
                if prev_data is None:
                    # Первая версия - full snapshot
                    delta = {k: {'old': None, 'new': v} for k, v in current_data.items()}
                else:
                    # Вычисляем delta
                    delta = {}
                    for field in set(list(prev_data.keys()) + list(current_data.keys())):
                        old_val = prev_data.get(field)
                        new_val = current_data.get(field)
                        
                        if old_val != new_val:
                            delta[field] = {'old': old_val, 'new': new_val}
                
                writer.writerow({
                    'content_type': revision['model'].lower(),
                    'content_id': f"{revision['model'].lower()}-{revision['object_id']}",
                    'delta': json.dumps(delta),
                    'created_at': revision['history_date'],
                    'created_by': revision['history_user']
                })
                
                prev_data = current_data

if __name__ == '__main__':
    transform_django_to_rustok(
        'django_history_export.csv',
        'rustok_import.csv'
    )
```

#### 4. Импорт в rustok-revisions

Используйте тот же скрипт импорта, что и для PaperTrail.

## Миграция с Django Reversion

### Обзор Django Reversion

**Django Reversion** — еще одна популярная Python библиотека для версионирования.

**Особенности:**
- Full snapshot storage
- Поддержка version sets
- Point-in-time recovery
- Admin integration

### Шаги миграции

Процесс аналогичен миграции с Django Simple History:

1. Экспорт из `reversion.models.Version`
2. Трансформация в delta format
3. Импорт в rustok-revisions

```python
# export_django_reversion.py
from reversion.models import Version
import csv
import json

with open('django_reversion_export.csv', 'w', newline='') as f:
    writer = csv.writer(f)
    writer.writerow(['content_type', 'object_id', 'data', 'date_created', 'user'])
    
    for version in Version.objects.all().order_by('date_created'):
        writer.writerow([
            version.content_type.model,
            version.object_id,
            json.dumps(version.field_dict),
            version.date_created.isoformat(),
            version.revision.user_id or 'system'
        ])
```

## Миграция с Git-based решений

### Обзор Git-based решений

Некоторые системы используют Git для хранения истории:
- Хранение контента как файлов
- Git commits как ревизии
- Git log как история

### Шаги миграции

#### 1. Экспорт из Git

```bash
#!/bin/bash
# export_git_history.sh

OUTPUT_FILE="git_history_export.csv"

echo "file_path,commit_hash,author,date,message,content" > $OUTPUT_FILE

for file in $(find content -name "*.md" -o -name "*.json"); do
    git log --pretty=format:"%H|%an|%ai|%s" --follow "$file" | while IFS='|' read hash author date message; do
        content=$(git show "$hash:$file" | base64)
        echo "$file,$hash,$author,$date,$message,$content" >> $OUTPUT_FILE
    done
done
```

#### 2. Трансформация

```python
# transform_git_to_rustok.py
import csv
import json
import base64
from collections import defaultdict

def transform_git_to_rustok(input_file, output_file):
    # Группируем по файлу
    files = defaultdict(list)
    
    with open(input_file, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            files[row['file_path']].append(row)
    
    # Сортируем по дате
    for file_path in files:
        files[file_path].sort(key=lambda x: x['date'])
    
    # Вычисляем deltas
    with open(output_file, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=[
            'content_type', 'content_id', 'delta', 'created_at', 'created_by', 'change_summary'
        ])
        writer.writeheader()
        
        for file_path, revisions in files.items():
            prev_content = None
            
            for revision in revisions:
                current_content = base64.b64decode(revision['content']).decode('utf-8')
                
                # Парсим контент (предполагаем JSON)
                try:
                    current_data = json.loads(current_content)
                except:
                    current_data = {'content': current_content}
                
                if prev_content is None:
                    delta = {k: {'old': None, 'new': v} for k, v in current_data.items()}
                else:
                    prev_data = json.loads(prev_content)
                    delta = {}
                    for field in set(list(prev_data.keys()) + list(current_data.keys())):
                        old_val = prev_data.get(field)
                        new_val = current_data.get(field)
                        if old_val != new_val:
                            delta[field] = {'old': old_val, 'new': new_val}
                
                # Определяем content_type из пути
                content_type = file_path.split('/')[1] if '/' in file_path else 'file'
                content_id = file_path.replace('/', '-').replace('.json', '').replace('.md', '')
                
                writer.writerow({
                    'content_type': content_type,
                    'content_id': content_id,
                    'delta': json.dumps(delta),
                    'created_at': revision['date'],
                    'created_by': revision['author'],
                    'change_summary': revision['message']
                })
                
                prev_content = current_content

if __name__ == '__main__':
    transform_git_to_rustok(
        'git_history_export.csv',
        'rustok_import.csv'
    )
```

## Миграция с Custom решений

### Общие принципы

Если вы используете custom решение для revision history:

1. **Экспортируйте данные** в универсальный формат (CSV, JSON)
2. **Трансформируйте** в delta format
3. **Импортируйте** в rustok-revisions

### Пример custom схемы

```sql
-- Custom revision table
CREATE TABLE custom_revisions (
    id SERIAL PRIMARY KEY,
    entity_type VARCHAR(255),
    entity_id INTEGER,
    old_values JSONB,
    new_values JSONB,
    changed_by INTEGER,
    changed_at TIMESTAMP
);
```

### Экспорт

```sql
-- Экспорт в CSV
COPY (
    SELECT 
        entity_type,
        entity_id,
        old_values,
        new_values,
        changed_by,
        changed_at
    FROM custom_revisions
    ORDER BY changed_at
) TO '/tmp/custom_revisions_export.csv' WITH CSV HEADER;
```

### Трансформация

```python
# transform_custom.py
import csv
import json

def transform_custom_to_rustok(input_file, output_file):
    with open(input_file, 'r') as infile, open(output_file, 'w', newline='') as outfile:
        reader = csv.DictReader(infile)
        writer = csv.DictWriter(outfile, fieldnames=[
            'content_type', 'content_id', 'delta', 'created_at', 'created_by'
        ])
        writer.writeheader()
        
        for row in reader:
            old_values = json.loads(row['old_values'] or '{}')
            new_values = json.loads(row['new_values'] or '{}')
            
            # Вычисляем delta
            delta = {}
            for field in set(list(old_values.keys()) + list(new_values.keys())):
                old_val = old_values.get(field)
                new_val = new_values.get(field)
                
                if old_val != new_val:
                    delta[field] = {'old': old_val, 'new': new_val}
            
            writer.writerow({
                'content_type': row['entity_type'].lower(),
                'content_id': f"{row['entity_type'].lower()}-{row['entity_id']}",
                'delta': json.dumps(delta),
                'created_at': row['changed_at'],
                'created_by': row['changed_by'] or 'system'
            })

if __name__ == '__main__':
    transform_custom_to_rustok(
        'custom_revisions_export.csv',
        'rustok_import.csv'
    )
```

## Общие принципы миграции

### 1. Планирование

**Перед миграцией:**

- [ ] Проанализируйте существующую схему
- [ ] Определите объем данных
- [ ] Оцените время миграции
- [ ] Создайте backup
- [ ] Подготовьте rollback plan

### 2. Data Mapping

**Маппинг полей:**

| Source Field | rustok-revisions Field | Notes |
|--------------|------------------------|-------|
| item_type / model | content_type | Lowercase |
| item_id / object_id | content_id | String format |
| object / data | delta | Transform to delta format |
| created_at / history_date | created_at | ISO 8601 |
| whodunnit / history_user | created_by | User ID |
| - | tenant_id | Add default |
| - | locale | Add if needed |
| - | version_name | Add for important versions |

### 3. Delta Format

**Стандартный delta format:**

```json
{
  "field_name": {
    "old": "old_value",
    "new": "new_value"
  },
  "another_field": {
    "old": 10,
    "new": 20
  }
}
```

**Для создания:**
```json
{
  "title": {"old": null, "new": "Hello"},
  "content": {"old": null, "new": "World"}
}
```

**Для удаления:**
```json
{
  "title": {"old": "Hello", "new": null},
  "content": {"old": "World", "new": null}
}
```

### 4. Batch Processing

**Для больших объемов данных:**

```rust
const BATCH_SIZE: usize = 1000;

async fn import_in_batches(
    service: &RevisionService,
    records: Vec<Revision>,
) -> Result<usize, RevisionError> {
    let mut imported = 0;
    
    for chunk in records.chunks(BATCH_SIZE) {
        let batch: Vec<_> = chunk.to_vec();
        service.batch_create_revisions(batch).await?;
        imported += chunk.len();
        
        println!("Imported {} records", imported);
    }
    
    Ok(imported)
}
```

## Data Migration Scripts

### Полный скрипт миграции

```rust
// migration_tool.rs
use clap::Parser;
use csv::ReaderBuilder;
use rustok_revisions::{RevisionService, SeaORMBackend};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "migration-tool")]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    
    #[arg(short, long)]
    database_url: String,
    
    #[arg(short, long, default_value = "default")]
    tenant_id: String,
    
    #[arg(long, default_value = "1000")]
    batch_size: usize,
}

#[derive(Debug, Deserialize)]
struct ImportRecord {
    content_type: String,
    content_id: String,
    delta: String,
    created_at: String,
    created_by: String,
    change_summary: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    println!("Starting migration from {:?}", args.input);
    
    // Connect to database
    let backend = SeaORMBackend::connect(&args.database_url).await?;
    let service = RevisionService::new(backend);
    
    // Read CSV
    let mut reader = ReaderBuilder::new().from_path(&args.input)?;
    let mut batch = Vec::new();
    let mut imported = 0;
    
    for result in reader.deserialize() {
        let record: ImportRecord = result?;
        
        let delta: serde_json::Value = serde_json::from_str(&record.delta)?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&record.created_at)?;
        
        let revision = Revision {
            id: Uuid::new_v4(),
            tenant_id: args.tenant_id.clone(),
            content_type: record.content_type,
            content_id: record.content_id,
            locale: None,
            revision_number: 0,
            parent_revision_id: None,
            delta,
            created_by: Some(record.created_by),
            created_at: created_at.with_timezone(&chrono::Utc),
            change_source: Some("migration".to_string()),
            change_summary: record.change_summary,
            version_name: None,
        };
        
        batch.push(revision);
        
        if batch.len() >= args.batch_size {
            service.batch_create_revisions(batch.clone()).await?;
            imported += batch.len();
            println!("Imported {} records", imported);
            batch.clear();
        }
    }
    
    // Import remaining
    if !batch.is_empty() {
        service.batch_create_revisions(batch).await?;
        imported += batch.len();
    }
    
    println!("Migration complete! Imported {} records", imported);
    
    Ok(())
}
```

### Использование

```bash
# Компиляция
cargo build --release --bin migration-tool

# Запуск
./target/release/migration-tool \
  --input rustok_import.csv \
  --database-url postgres://rustok:password@localhost:5432/rustok_revisions \
  --tenant-id default \
  --batch-size 1000
```

## Verification

### 1. Проверка количества записей

```sql
-- Общее количество
SELECT count(*) FROM content_revisions;

-- По типам контента
SELECT content_type, count(*) 
FROM content_revisions 
GROUP BY content_type 
ORDER BY count(*) DESC;

-- По датам
SELECT 
    date_trunc('month', created_at) as month,
    count(*) as revisions
FROM content_revisions
GROUP BY month
ORDER BY month;
```

### 2. Проверка целостности данных

```rust
// verify_migration.rs
async fn verify_migration(
    service: &RevisionService,
    expected_counts: HashMap<String, usize>,
) -> Result<(), RevisionError> {
    for (content_type, expected_count) in expected_counts {
        let actual_count = service.count_revisions(&content_type).await?;
        
        if actual_count != expected_count {
            eprintln!(
                "Mismatch for {}: expected {}, got {}",
                content_type, expected_count, actual_count
            );
        } else {
            println!("✓ {}: {} revisions", content_type, actual_count);
        }
    }
    
    Ok(())
}
```

### 3. Проверка восстановления

```rust
// test_restoration.rs
async fn test_restoration(
    service: &RevisionService,
    test_cases: Vec<(&str, i32)>,  // (content_id, revision_number)
) -> Result<(), RevisionError> {
    for (content_id, revision_number) in test_cases {
        let revision = service.get_revision::<Post>(
            content_id,
            None,
            revision_number,
        ).await?;
        
        match revision {
            Some(rev) => {
                println!("✓ {} revision {} exists", content_id, revision_number);
                println!("  Delta: {:?}", rev.delta);
            }
            None => {
                eprintln!("✗ {} revision {} not found", content_id, revision_number);
            }
        }
    }
    
    Ok(())
}
```

### 4. Сравнение с оригиналом

```python
# compare_migration.py
import csv
import json

def compare_migration(original_file, migrated_file):
    original = {}
    migrated = {}
    
    # Load original
    with open(original_file, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = f"{row['content_type']}-{row['content_id']}"
            original[key] = original.get(key, 0) + 1
    
    # Load migrated
    with open(migrated_file, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = f"{row['content_type']}-{row['content_id']}"
            migrated[key] = migrated.get(key, 0) + 1
    
    # Compare
    all_keys = set(list(original.keys()) + list(migrated.keys()))
    
    mismatches = 0
    for key in all_keys:
        orig_count = original.get(key, 0)
        mig_count = migrated.get(key, 0)
        
        if orig_count != mig_count:
            print(f"✗ {key}: original={orig_count}, migrated={mig_count}")
            mismatches += 1
    
    if mismatches == 0:
        print(f"✓ All {len(all_keys)} objects match!")
    else:
        print(f"✗ {mismatches} mismatches found")

if __name__ == '__main__':
    compare_migration(
        'original_export.csv',
        'rustok_import.csv'
    )
```

## Rollback Plan

### Если что-то пошло не так

**1. Остановите приложение:**
```bash
sudo systemctl stop rustok-revisions
```

**2. Удалите импортированные данные:**
```sql
DELETE FROM content_revisions 
WHERE change_source = 'migration';
```

**3. Восстановите из backup:**
```bash
pg_restore -U rustok -d rustok_revisions -c backup_before_migration.sql
```

**4. Исправьте проблемы и повторите миграцию**

## Заключение

Этот migration guide покрывает:

✅ **PaperTrail** — полная миграция с Ruby  
✅ **Django Simple History** — миграция с Python  
✅ **Django Reversion** — миграция с Python  
✅ **Git-based** — миграция из Git  
✅ **Custom решения** — общие принципы  
✅ **Data migration scripts** — готовые скрипты  
✅ **Verification** — проверка целостности  

**Ключевые шаги:**

1. Экспорт данных из старой системы
2. Трансформация в delta format
3. Импорт в rustok-revisions
4. Верификация целостности
5. Тестирование восстановления

**Удачи в миграции!** 🚀
