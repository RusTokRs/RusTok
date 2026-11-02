/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { getProfileInitials } from "./avatar.ts";
import {
  fetchProfileByHandle,
  loadProfilesStorefrontPage,
  setProfilesStorefrontFollow,
  PROFILE_QUERY,
  FOLLOW_STATE_QUERY,
} from "./api.ts";

test("getProfileInitials handles various displayName and handle formats", () => {
  assert.equal(getProfileInitials({ displayName: "Alice Smith", handle: "alice" }), "AS");
  assert.equal(getProfileInitials({ displayName: "Bob", handle: "bob" }), "B");
  assert.equal(getProfileInitials({ displayName: "  Иван   Петров  ", handle: "ivan" }), "ИП");
  assert.equal(getProfileInitials({ displayName: "   ", handle: "charlie" }), "C");
  assert.equal(getProfileInitials({ displayName: "", handle: "" }), "?");
});

test("loadProfilesStorefrontPage marks isSelf and skips follow state for own profile", async () => {
  const mockGraphql = async ({ query }) => {
    if (query === PROFILE_QUERY) {
      return {
        data: {
          profile: {
            userId: "user-123",
            handle: "alice",
            displayName: "Alice",
            bio: "Developer",
            tags: ["rust", "nextjs"],
            avatarMediaId: null,
            bannerMediaId: null,
            avatarImage: null,
            bannerImage: null,
            preferredLocale: "en",
            visibility: "public",
          },
        },
      };
    }
    throw new Error("Should not fetch follow state for self");
  };

  const page = await loadProfilesStorefrontPage(
    mockGraphql,
    "alice",
    "en",
    "valid-jwt-token",
    null,
    "user-123" // same as profile.userId
  );

  assert.ok(page.profile);
  assert.equal(page.isSelf, true);
  assert.equal(page.viewerAuthenticated, true);
  assert.equal(page.followState, null);
});

test("loadProfilesStorefrontPage fetches follow state for other users when authenticated", async () => {
  const mockGraphql = async ({ query }) => {
    if (query === PROFILE_QUERY) {
      return {
        data: {
          profile: {
            userId: "user-456",
            handle: "bob",
            displayName: "Bob",
            bio: null,
            tags: [],
            avatarMediaId: null,
            bannerMediaId: null,
            avatarImage: null,
            bannerImage: null,
            preferredLocale: "en",
            visibility: "public",
          },
        },
      };
    }
    if (query === FOLLOW_STATE_QUERY) {
      return {
        data: {
          state: {
            userId: "user-456",
            following: true,
            revision: "rev-1",
          },
        },
      };
    }
    return { data: {} };
  };

  const page = await loadProfilesStorefrontPage(
    mockGraphql,
    "bob",
    "en",
    "valid-jwt-token",
    null,
    "user-123" // different viewer
  );

  assert.ok(page.profile);
  assert.equal(page.isSelf, false);
  assert.equal(page.viewerAuthenticated, true);
  assert.ok(page.followState);
  assert.equal(page.followState.following, true);
  assert.equal(page.followState.revision, "rev-1");
});

test("setProfilesStorefrontFollow and fetchProfileByHandle throw on GraphQL errors (anti-swallowing)", async () => {
  const mockFailingGraphql = async () => ({
    errors: [{ message: "Mutation rejected" }],
  });

  await assert.rejects(
    async () => {
      await fetchProfileByHandle(mockFailingGraphql, "charlie");
    },
    {
      name: "Error",
      message: "Mutation rejected",
    }
  );

  await assert.rejects(
    async () => {
      await setProfilesStorefrontFollow(mockFailingGraphql, {
        userId: "user-1",
        following: true,
        idempotencyKey: "idem-1",
      });
    },
    {
      name: "Error",
      message: "Mutation rejected",
    }
  );
});
