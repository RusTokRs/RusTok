---
id: doc://docs/concepts/social-modules.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Social Modules Concept

Ниже — единый, вычищенный “Master Concept” модульной архитектуры соцсети (Gemini + Opus), без внутренних противоречий и с чётким ответом на главный вопрос: сколько crate’ов, какие слои, какие границы, как общаются, кто чем владеет.

Цель платформы: гибрид ВКонтакте-социальности + phpFox-модульных групп + Discourse-форумного ядра.

1) Архитектурная доктрина
1.1. Формат: Modular Monolith (Rust Workspace)

Каждый модуль = отдельный crate (crates/mod_*) с явными публичными портами (traits) и приватным хранилищем.

Никаких “жёстких” зависимостей домена на домен: модуль не импортирует чужие модели/таблицы.

Общение модулей:

Sync Ports (Traits) — чтение/проверки/доступ: “можно ли?”, “дай summary”, “проверь связь”.

Async Events (Event Bus) — бизнес-последствия: “создан пост” → “добавить в feed” + “начислить карму” + “уведомить”.

1.2. Два режима “счётчика модулей”

В ваших двух концептах спор не про “правильность”, а про гранулярность. Я фиксирую рекомендуемый вариант и даю безопасный “переключатель”:

Рекомендуемый (чистый): 13 crate’ов (лучше для сопровождения: Trust/Karma отдельно, Moderation отдельно, Media Social отдельно).

Строгий “12 модулей”: можно слить mod_reputation + mod_moderation в единый mod_engagement или встроить mod_events в mod_groups как feature-подмодуль.
Но дальше я описываю нормальную финальную схему на 13 — она лучше стыкует Gemini и Opus без потерь.

2) Финальная карта модулей (13 crates) и слои
LAYER 0: CMS CORE (уже есть / инфраструктура)

Users/Auth, Media Storage, Search, Pages/Blog, Notifications, Feature Flags, Settings

LAYER 1: FOUNDATION (базовые сервисы)

mod_profiles — расширенные профили + privacy matrix + online/verification

mod_social_graph — дружбы/подписки/блокировки/списки/запросы

mod_reputation — Trust Levels + Karma + правила начисления

LAYER 2: CROSS-CUTTING (полиморфные “сквозные”)

mod_reactions — универсальные реакции для любых сущностей

mod_moderation — репорты/страйки/баны/automod (вес жалоб учитывает Trust)

LAYER 3: CONTENT (генераторы контента)

mod_wall — посты + комментарии + закрепы + вложения (activity stream)

mod_forum — категории/темы/посты/вики/решения (discourse-ядро)

mod_media_social — альбомы, теги людей на фото, плейлисты (VK/PHPfox must-have)

LAYER 4: CONTAINERS (пространства/контексты)

mod_groups — группы, участники, роли, invites, bans, BitMask фич

mod_events — события, RSVP, приглашения, напоминания

mod_market — объявления/листинги/магазины/отзывы

LAYER 5: AGGREGATION (точки входа / UX-склейка)

mod_feed — лента (fan-out on write + ранжирование) + mute-источники

mod_im — мессенджер (диалоги/чаты/статусы/поиск)

3) Контракты общения модулей
3.1. Универсальные идентификаторы сущностей (единый язык)

Минимальный общий тип, который понимают сквозные модули:

#[derive(Clone, Copy)]
pub struct EntityRef {
    pub entity_type: u16, // Post=1, Comment=2, Topic=3, Album=4, Listing=5, ...
    pub entity_id: u64,
}

3.2. Порты (traits) — только то, что нужно “снаружи”

Примеры обязательных портов (ядро всей связности):

ProfilesReader: аватар/имя/приватность поля/верификация

SocialGraphReader: is_friend/is_blocked/get_followers/get_friends

ReputationReader: trust_level(user), can(action), karma(user)

GroupsAccess: is_member/can_post/can_view + enabled_features(group_id)

ModerationReader: is_banned(scope), can_report(weight), automod_check(text)

Важно: Content-модули (wall/forum/media) не должны знать таблицы groups, они знают только порт GroupsAccess.

3.3. Event Bus — “клей” без циклических зависимостей

События генерируют модули, а реагируют агрегаторы/контроль.

Ключевые доменные события (минимальный набор):

PostCreated, CommentCreated

TopicCreated, TopicReplied, SolutionMarked, WikiEdited

AlbumCreated, MediaAddedToAlbum, PersonTagged

RelationChanged

ReactionAdded/Removed

ReportCreated/Resolved, UserBanned/Warned

GroupFeatureToggled, MemberJoined/Left

EventCreated, AttendanceChanged

ListingCreated/Sold, ReviewCreated

MessageSent/Read

Матрица подписок (самая практичная):

mod_feed слушает: всё “создано” (пост/тема/альбом/листинг/ивент) → добавляет в ленты

mod_reputation слушает: посты/комменты/темы/реакции/решения/подтверждённые репорты → начисляет/списывает

CMS Notifications слушает: почти всё → пуши/инбокс/дайджест

mod_moderation слушает: входящий контент + сообщения (опционально) → automod

4) Границы владения данными (чтобы не превратилось в “комок”)

Жёсткое правило: каждый модуль владеет своими таблицами (лучше — отдельной схемой в PostgreSQL):

profiles.* — расширенный профиль/приватность/онлайн/верификация

social_graph.* — связи/списки/заявки/блоки

reputation.* — karma, trust levels, правила, история

reactions.* — реакции, типы реакций

moderation.* — репорты/баны/страйки/правила/аудит

wall.* — посты/комменты/вложения/закрепы

forum.* — категории/темы/посты/вики-ревизии/решения

media_social.* — альбомы/элементы/теги людей/плейлисты

groups.* — группы/участники/роли/инвайты/баны/feature mask

events.* — события/участники/инвайты/ремайндеры

market.* — листинги/магазины/отзывы/категории

feed.* — feed_items/cache/muted/settings

im.* — диалоги/участники/сообщения/статусы

Запрещено: mod_feed лезет в wall.posts напрямую.
Разрешено: mod_feed хранит только (EntityRef, score, context) и при рендере дёргает порт WallReader/ForumReader/MediaReader.

5) Особые “фишки” из Gemini, которые фиксируем как стандарт
5.1. BitMask фич группы (быстро и расширяемо)

mod_groups хранит enabled_features: u32, где биты — включённые модули:

WALL, FORUM, MEDIA, EVENTS, MARKET, WIKI, CHAT …

Плюсы:

мгновенная проверка в рантайме

простая сериализация и миграции

фронтенд получает has_forum/has_market без сложных запросов

5.2. Trait-based полиморфизм (Reactable/Reportable)

Сквозные модули оперируют только EntityRef, и им не важно “это пост или товар”.

6) Зависимости слоёв (что кому можно импортировать)

Правило: слои зависят “вниз”, но общаются “вверх” через события.

Content может зависеть на Foundation порты (GroupsAccess, ProfilesReader, ReputationReader).

Cross-cutting не зависит от Content (он универсален).

Aggregation может зависеть на всех (как “оркестратор”), но не должен владеть их моделями.

7) Порядок реализации без боли (по фазам)

Фаза 1 — фундамент (чтобы всё остальное не развалилось):

mod_profiles (privacy + identity surface)

mod_social_graph (доступ/блокировки)

mod_reputation (trust gating)

Фаза 2 — сквозные контуры безопасности/UX:
4) mod_reactions
5) mod_moderation

Фаза 3 — контентное ядро:
6) mod_wall
7) mod_forum
8) mod_media_social

Фаза 4 — контейнеры:
9) mod_groups (с BitMask фич)
10) mod_events
11) mod_market

Фаза 5 — пользовательская “магия”:
12) mod_feed (fan-out + ранжирование)
13) mod_im

8) Если всё-таки хотите ровно “12 модулей” (без вопросов, просто вариант)

Самый безопасный компромисс (минимум потерь чистоты):

Слить mod_reputation + mod_moderation → mod_engagement (как у Gemini: “наблюдатель/каратель”)
И получите 12 crate’ов, сохранив отдельный mod_media_social.
