# GraphQL API for Content Revision History

**Статус:** Design Document  
**Дата:** 2026-10-09

## Overview

Этот документ описывает GraphQL API для работы с revision history в RusTok platform.

## Schema

### Types

```graphql
"""
Revision represents a single change to content
"""
type Revision {
  """Unique identifier"""
  id: ID!
  
  """Revision number (auto-incrementing per content item)"""
  revisionNumber: Int!
  
  """Tenant ID"""
  tenantId: UUID!
  
  """Content type (e.g., "blog_post", "forum_topic")"""
  contentType: String!
  
  """Content ID"""
  contentId: UUID!
  
  """Locale for multilingual content"""
  locale: String!
  
  """Parent revision ID"""
  parentRevisionId: UUID
  
  """Delta containing changed fields with old and new values"""
  delta: JSON!
  
  """User who made the change"""
  createdBy: User!
  
  """When the revision was created"""
  createdAt: DateTime!
  
  """Source of the change"""
  changeSource: ChangeSource!
  
  """Optional summary of the change"""
  changeSummary: String
  
  """Optional named version (snapshot name)"""
  versionName: String
}

"""
Source of a content change
"""
enum ChangeSource {
  ADMIN_UI
  API
  IMPORT
  RESTORE
  OTHER
}

"""
Field change in a revision
"""
type FieldChange {
  """Field name"""
  field: String!
  
  """Old value (before change)"""
  oldValue: JSON
  
  """New value (after change)"""
  newValue: JSON
}

"""
Diff between two revisions
"""
type RevisionDiff {
  """From revision number"""
  fromRevision: Int!
  
  """To revision number"""
  toRevision: Int!
  
  """List of field changes"""
  changes: [FieldChange!]!
  
  """Summary of changes"""
  summary: String!
  
  """Human-readable description"""
  humanReadable: String!
  
  """Number of changed fields"""
  changedFieldsCount: Int!
  
  """Whether there are any changes"""
  hasChanges: Boolean!
}

"""
Named version (snapshot)
"""
type NamedVersion {
  """Revision"""
  revision: Revision!
  
  """Version name"""
  name: String!
  
  """When the version was created"""
  createdAt: DateTime!
  
  """User who created the version"""
  createdBy: User!
}

"""
Paginated list of revisions
"""
type RevisionConnection {
  """List of revisions"""
  edges: [RevisionEdge!]!
  
  """Pagination info"""
  pageInfo: PageInfo!
  
  """Total count"""
  totalCount: Int!
}

type RevisionEdge {
  """Revision"""
  node: Revision!
  
  """Cursor for pagination"""
  cursor: String!
}

type PageInfo {
  """Whether there are more pages"""
  hasNextPage: Boolean!
  
  """Whether there are previous pages"""
  hasPreviousPage: Boolean!
  
  """Cursor for the first edge"""
  startCursor: String
  
  """Cursor for the last edge"""
  endCursor: String
}

"""
Input for creating a named version
"""
input CreateNamedVersionInput {
  """Content ID"""
  contentId: UUID!
  
  """Content type"""
  contentType: String!
  
  """Locale"""
  locale: String!
  
  """Version name"""
  versionName: String!
  
  """Optional summary"""
  summary: String
}

"""
Input for restoring a revision
"""
input RestoreRevisionInput {
  """Content ID"""
  contentId: UUID!
  
  """Content type"""
  contentType: String!
  
  """Locale"""
  locale: String!
  
  """Target revision number"""
  revisionNumber: Int!
}
```

### Queries

```graphql
extend type Query {
  """
  Get revision by ID
  """
  revision(id: ID!): Revision
  
  """
  List revisions for a content item
  """
  revisions(
    """Content ID"""
    contentId: UUID!
    
    """Content type"""
    contentType: String!
    
    """Locale"""
    locale: String!
    
    """First N items"""
    first: Int
    
    """After cursor"""
    after: String
    
    """Last N items"""
    last: Int
    
    """Before cursor"""
    before: String
  ): RevisionConnection!
  
  """
  Get diff between two revisions
  """
  revisionDiff(
    """Content ID"""
    contentId: UUID!
    
    """Content type"""
    contentType: String!
    
    """Locale"""
    locale: String!
    
    """From revision number"""
    fromRevision: Int!
    
    """To revision number"""
    toRevision: Int!
  ): RevisionDiff!
  
  """
  List all named versions for a content item
  """
  namedVersions(
    """Content ID"""
    contentId: UUID!
    
    """Content type"""
    contentType: String!
    
    """Locale"""
    locale: String!
  ): [NamedVersion!]!
  
  """
  Check if revision tracking is enabled for a content type
  """
  isRevisionTrackingEnabled(
    """Content type"""
    contentType: String!
  ): Boolean!
}
```

### Mutations

```graphql
extend type Mutation {
  """
  Restore content to a previous revision
  """
  restoreRevision(input: RestoreRevisionInput!): RestoreRevisionPayload!
  
  """
  Create a named version (snapshot)
  """
  createNamedVersion(input: CreateNamedVersionInput!): CreateNamedVersionPayload!
  
  """
  Delete a named version
  """
  deleteNamedVersion(
    """Revision ID"""
    revisionId: ID!
  ): DeleteNamedVersionPayload!
}

type RestoreRevisionPayload {
  """Restored content (as JSON)"""
  content: JSON!
  
  """New revision created for the restore"""
  revision: Revision!
  
  """Success flag"""
  success: Boolean!
  
  """Error message if failed"""
  error: String
}

type CreateNamedVersionPayload {
  """Created named version"""
  namedVersion: NamedVersion!
  
  """Success flag"""
  success: Boolean!
  
  """Error message if failed"""
  error: String
}

type DeleteNamedVersionPayload {
  """Success flag"""
  success: Boolean!
  
  """Error message if failed"""
  error: String
}
```

## Extending Content Types

### BlogPost Example

```graphql
extend type BlogPost {
  """
  Get revision history for this post
  """
  revisions(
    """Locale"""
    locale: String = "en"
    
    """First N items"""
    first: Int
    
    """After cursor"""
    after: String
  ): RevisionConnection!
  
  """
  Get diff between two revisions
  """
  revisionDiff(
    """From revision number"""
    fromRevision: Int!
    
    """To revision number"""
    toRevision: Int!
    
    """Locale"""
    locale: String = "en"
  ): RevisionDiff!
  
  """
  Get named versions for this post
  """
  namedVersions(
    """Locale"""
    locale: String = "en"
  ): [NamedVersion!]!
  
  """
  Restore to a specific revision
  """
  restoreRevision(
    """Target revision number"""
    revisionNumber: Int!
  ): BlogPost!
  
  """
  Create a named version
  """
  createNamedVersion(
    """Version name"""
    name: String!
    
    """Optional summary"""
    summary: String
  ): NamedVersion!
}
```

## Resolvers

### Query Resolvers

```rust
use async_graphql::*;
use rustok_content_revisions::ContentRevisionService;
use std::sync::Arc;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn revision(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<Revision>> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let revision_id = Uuid::parse_str(&id.to_string())?;
        
        match service.get_revision(revision_id).await {
            Ok(revision) => Ok(Some(revision.into())),
            Err(_) => Ok(None),
        }
    }
    
    async fn revisions(
        &self,
        ctx: &Context<'_>,
        content_id: Uuid,
        content_type: String,
        locale: String,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<RevisionConnection> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        
        let revisions = service.list_revisions(
            *tenant_id,
            &content_type,
            content_id,
            &locale,
        ).await?;
        
        // Apply pagination
        let limit = first.unwrap_or(20) as usize;
        let offset = after
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);
        
        let page_revisions: Vec<Revision> = revisions
            .into_iter()
            .skip(offset)
            .take(limit + 1)
            .collect();
        
        let has_next_page = page_revisions.len() > limit;
        let edges: Vec<RevisionEdge> = page_revisions
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, r)| RevisionEdge {
                node: r.into(),
                cursor: (offset + i).to_string(),
            })
            .collect();
        
        Ok(RevisionConnection {
            edges,
            page_info: PageInfo {
                has_next_page,
                has_previous_page: offset > 0,
                start_cursor: edges.first().map(|e| e.cursor.clone()),
                end_cursor: edges.last().map(|e| e.cursor.clone()),
            },
            total_count: 0, // TODO: Get actual count
        })
    }
    
    async fn revision_diff(
        &self,
        ctx: &Context<'_>,
        content_id: Uuid,
        content_type: String,
        locale: String,
        from_revision: i32,
        to_revision: i32,
    ) -> Result<RevisionDiff> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        
        let diff = service.diff_revisions(
            *tenant_id,
            &content_type,
            content_id,
            &locale,
            from_revision,
            to_revision,
        ).await?;
        
        Ok(diff.into())
    }
    
    async fn named_versions(
        &self,
        ctx: &Context<'_>,
        content_id: Uuid,
        content_type: String,
        locale: String,
    ) -> Result<Vec<NamedVersion>> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        
        let versions = service.list_named_versions(
            *tenant_id,
            &content_type,
            content_id,
            &locale,
        ).await?;
        
        Ok(versions.into_iter().map(|r| r.into()).collect())
    }
    
    async fn is_revision_tracking_enabled(
        &self,
        ctx: &Context<'_>,
        content_type: String,
    ) -> Result<bool> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        Ok(service.is_enabled(&content_type))
    }
}
```

### Mutation Resolvers

```rust
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        input: RestoreRevisionInput,
    ) -> Result<RestoreRevisionPayload> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let user_id = ctx.data::<Uuid>()?;
        
        // Get current content
        let current = get_current_content(
            &input.content_type,
            input.content_id,
            &input.locale,
        ).await?;
        
        // Restore
        let restored = service.restore_revision(
            *tenant_id,
            input.content_id,
            &input.locale,
            input.revision_number,
            &current,
            *user_id,
        ).await?;
        
        // Get the new revision created by restore
        let revisions = service.list_revisions(
            *tenant_id,
            &input.content_type,
            input.content_id,
            &input.locale,
        ).await?;
        
        let revision = revisions.first().unwrap().clone();
        
        Ok(RestoreRevisionPayload {
            content: serde_json::to_value(&restored)?,
            revision: revision.into(),
            success: true,
            error: None,
        })
    }
    
    async fn create_named_version(
        &self,
        ctx: &Context<'_>,
        input: CreateNamedVersionInput,
    ) -> Result<CreateNamedVersionPayload> {
        let service = ctx.data::<Arc<ContentRevisionService>>()?;
        let tenant_id = ctx.data::<Uuid>()?;
        let user_id = ctx.data::<Uuid>()?;
        
        // Get current content
        let current = get_current_content(
            &input.content_type,
            input.content_id,
            &input.locale,
        ).await?;
        
        // Create named version
        let revision = service.create_named_version(
            *tenant_id,
            input.content_id,
            &input.locale,
            &current,
            &input.version_name,
            *user_id,
        ).await?;
        
        Ok(CreateNamedVersionPayload {
            named_version: revision.into(),
            success: true,
            error: None,
        })
    }
}
```

### BlogPost Extension

```rust
use crate::entities::blog_post;

#[Object]
impl BlogPost {
    // ... existing fields
    
    async fn revisions(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<RevisionConnection> {
        let query = ctx.data::<QueryRoot>()?;
        query.revisions(
            ctx,
            self.id,
            "blog_post".to_string(),
            locale.unwrap_or_else(|| "en".to_string()),
            first,
            after,
        ).await
    }
    
    async fn revision_diff(
        &self,
        ctx: &Context<'_>,
        from_revision: i32,
        to_revision: i32,
        locale: Option<String>,
    ) -> Result<RevisionDiff> {
        let query = ctx.data::<QueryRoot>()?;
        query.revision_diff(
            ctx,
            self.id,
            "blog_post".to_string(),
            locale.unwrap_or_else(|| "en".to_string()),
            from_revision,
            to_revision,
        ).await
    }
    
    async fn named_versions(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
    ) -> Result<Vec<NamedVersion>> {
        let query = ctx.data::<QueryRoot>()?;
        query.named_versions(
            ctx,
            self.id,
            "blog_post".to_string(),
            locale.unwrap_or_else(|| "en".to_string()),
        ).await
    }
    
    async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        revision_number: i32,
    ) -> Result<BlogPost> {
        let mutation = ctx.data::<MutationRoot>()?;
        let result = mutation.restore_revision(
            ctx,
            RestoreRevisionInput {
                content_id: self.id,
                content_type: "blog_post".to_string(),
                locale: "en".to_string(),
                revision_number,
            },
        ).await?;
        
        if !result.success {
            return Err(Error::new(result.error.unwrap_or_default()));
        }
        
        // Return updated post
        get_blog_post(self.id).await
    }
    
    async fn create_named_version(
        &self,
        ctx: &Context<'_>,
        name: String,
        summary: Option<String>,
    ) -> Result<NamedVersion> {
        let mutation = ctx.data::<MutationRoot>()?;
        let result = mutation.create_named_version(
            ctx,
            CreateNamedVersionInput {
                content_id: self.id,
                content_type: "blog_post".to_string(),
                locale: "en".to_string(),
                version_name: name,
                summary,
            },
        ).await?;
        
        if !result.success {
            return Err(Error::new(result.error.unwrap_or_default()));
        }
        
        Ok(result.named_version)
    }
}
```

## Usage Examples

### Query Revision History

```graphql
query {
  blogPost(id: "123") {
    title
    revisions(locale: "en", first: 10) {
      edges {
        node {
          revisionNumber
          createdAt
          createdBy {
            name
            email
          }
          changeSource
          changeSummary
          delta
        }
        cursor
      }
      pageInfo {
        hasNextPage
        endCursor
      }
      totalCount
    }
  }
}
```

### Compare Revisions

```graphql
query {
  blogPost(id: "123") {
    revisionDiff(fromRevision: 1, toRevision: 3, locale: "en") {
      fromRevision
      toRevision
      changes {
        field
        oldValue
        newValue
      }
      summary
      humanReadable
      changedFieldsCount
    }
  }
}
```

### Restore to Previous Version

```graphql
mutation {
  restoreRevision(input: {
    contentId: "123",
    contentType: "blog_post",
    locale: "en",
    revisionNumber: 5
  }) {
    success
    revision {
      revisionNumber
      createdAt
      changeSummary
    }
    error
  }
}
```

### Create Named Version

```graphql
mutation {
  createNamedVersion(input: {
    contentId: "123",
    contentType: "blog_post",
    locale: "en",
    versionName: "v1.0-published",
    summary: "Published version"
  }) {
    success
    namedVersion {
      name
      createdAt
      createdBy {
        name
      }
    }
    error
  }
}
```

### List Named Versions

```graphql
query {
  blogPost(id: "123") {
    namedVersions(locale: "en") {
      name
      createdAt
      createdBy {
        name
      }
      revision {
        revisionNumber
      }
    }
  }
}
```

## Admin UI Integration

### Revision History Panel

```jsx
function RevisionHistoryPanel({ postId }) {
  const { data, loading } = useQuery(GET_REVISIONS, {
    variables: { contentId: postId, contentType: "blog_post", locale: "en" }
  });
  
  if (loading) return <Spinner />;
  
  return (
    <div>
      <h3>Revision History</h3>
      {data.revisions.edges.map(({ node }) => (
        <RevisionItem key={node.revisionNumber} revision={node} />
      ))}
    </div>
  );
}
```

### Diff Viewer

```jsx
function DiffViewer({ postId, fromRevision, toRevision }) {
  const { data } = useQuery(GET_DIFF, {
    variables: {
      contentId: postId,
      contentType: "blog_post",
      locale: "en",
      fromRevision,
      toRevision
    }
  });
  
  return (
    <div>
      <h3>Changes from v{fromRevision} to v{toRevision}</h3>
      {data.revisionDiff.changes.map(change => (
        <FieldDiff key={change.field} change={change} />
      ))}
    </div>
  );
}
```

## Conclusion

GraphQL API для revision history готов к реализации. Следуйте этому дизайну для создания unified API для работы с revision history в RusTok platform.
