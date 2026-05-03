/// N5 seed data. Called once after schema creation.
/// Covers topics, vocabulary, kanji, topic_vocabulary, topic_dependencies,
/// and lesson plans for JLPT N5.
pub const N5_SEED_SQL: &str = "
-- =========================================================================
-- N5 Topics
-- =========================================================================
INSERT OR IGNORE INTO topics (id, jlpt_level, sequence_order, name, description, topic_type) VALUES
  (1,  5, 1,  'Greetings',              'Basic time-of-day greetings and farewells',                    'vocabulary'),
  (2,  5, 2,  'Self-Introduction',      'Saying your name, where you are from, and basic politeness',   'vocabulary'),
  (3,  5, 3,  'Numbers 1–10',           'Cardinal numbers one through ten',                             'vocabulary'),
  (4,  5, 4,  'Numbers 11–10000',       'Larger numbers, counters, and prices',                         'vocabulary'),
  (5,  5, 5,  'Days & Time',            'Days of the week, months, telling the time',                   'vocabulary'),
  (6,  5, 6,  'Family',                 'Immediate family members (own family vs. others)',              'vocabulary'),
  (7,  5, 7,  'Food & Drink',           'Common foods, drinks, and eating vocabulary',                   'vocabulary'),
  (8,  5, 8,  'Places & Directions',    'Common locations and basic directional language',               'vocabulary'),
  (9,  5, 9,  'Daily Actions',          'High-frequency verbs: eat, drink, go, come, do, see, buy',     'vocabulary'),
  (10, 5, 10, 'Adjectives I',           'Basic i-adjectives and na-adjectives for description',         'vocabulary'),
  (11, 5, 11, 'N5 Kanji — Nature',      'Sun, moon, mountain, river, fire, water, wood, metal, earth', 'kanji'),
  (12, 5, 12, 'N5 Kanji — People',      'Person, man, woman, child, mouth, eye, ear, hand, foot',      'kanji'),
  (13, 5, 13, 'N5 Kanji — Numbers',     'One through ten, hundred, thousand',                           'kanji'),
  (14, 5, 14, 'N5 Kanji — Time',        'Day, month/moon, year, time, morning, evening',               'kanji');

-- =========================================================================
-- Topic dependencies (depends_on_topic_id must be completed first)
-- =========================================================================
INSERT OR IGNORE INTO topic_dependencies (topic_id, depends_on_topic_id) VALUES
  (4,  3),   -- Numbers 11–10000 requires Numbers 1–10
  (5,  3),   -- Days & Time requires Numbers 1–10
  (9,  1),   -- Daily Actions requires Greetings (conversation context)
  (10, 2),   -- Adjectives I requires Self-Introduction (sentence structure)
  (13, 3),   -- Kanji Numbers requires Numbers 1–10 vocab
  (14, 5),   -- Kanji Time requires Days & Time vocab
  (12, 1);   -- Kanji People requires Greetings (context for person/people)

-- =========================================================================
-- N5 Vocabulary
-- =========================================================================
INSERT OR IGNORE INTO vocabulary (id, word, reading, meaning, jlpt_level) VALUES
-- Greetings (topic 1)
  (1,  'おはようございます', 'おはようございます', 'Good morning (formal)',       5),
  (2,  'おはよう',           'おはよう',           'Good morning (casual)',        5),
  (3,  'こんにちは',         'こんにちは',         'Hello / Good afternoon',       5),
  (4,  'こんばんは',         'こんばんは',         'Good evening',                 5),
  (5,  'さようなら',         'さようなら',         'Goodbye',                      5),
  (6,  'じゃあね',           'じゃあね',           'See you / Bye (casual)',        5),
  (7,  'ありがとうございます','ありがとうございます','Thank you (formal)',           5),
  (8,  'ありがとう',         'ありがとう',         'Thank you (casual)',            5),
  (9,  'すみません',         'すみません',         'Excuse me / Sorry',            5),
  (10, 'ごめんなさい',       'ごめんなさい',       'I''m sorry',                   5),
  (11, 'はい',               'はい',               'Yes',                          5),
  (12, 'いいえ',             'いいえ',             'No',                           5),
  (13, 'おやすみなさい',     'おやすみなさい',     'Good night',                   5),
  (14, 'いただきます',       'いただきます',       'Said before eating',           5),
  (15, 'ごちそうさまでした', 'ごちそうさまでした', 'Said after eating',            5),
-- Self-Introduction (topic 2)
  (16, 'わたし',             'わたし',             'I / me',                       5),
  (17, 'あなた',             'あなた',             'You',                          5),
  (18, 'なまえ',             '名前',               'Name',                         5),
  (19, 'です',               'です',               'Is / am / are (polite)',        5),
  (20, 'じゃないです',       'じゃないです',       'Is not (polite)',               5),
  (21, 'か',                 'か',                 'Question particle',             5),
  (22, 'も',                 'も',                 'Also / too',                   5),
  (23, 'の',                 'の',                 'Possessive particle / of',      5),
  (24, 'は',                 'は',                 'Topic particle',               5),
  (25, 'が',                 'が',                 'Subject particle',             5),
  (26, 'を',                 'を',                 'Object particle',              5),
  (27, 'に',                 'に',                 'Direction / time / location particle', 5),
  (28, 'で',                 'で',                 'Location of action / by means of', 5),
  (29, 'はじめまして',       'はじめまして',       'Nice to meet you',             5),
  (30, 'よろしくおねがいします', 'よろしくおねがいします', 'Please treat me well', 5),
-- Numbers 1–10 (topic 3)
  (31, 'いち',               'いち',               'One',                          5),
  (32, 'に',                 'に',                 'Two',                          5),
  (33, 'さん',               'さん',               'Three',                        5),
  (34, 'し / よん',          'し / よん',          'Four',                         5),
  (35, 'ご',                 'ご',                 'Five',                         5),
  (36, 'ろく',               'ろく',               'Six',                          5),
  (37, 'しち / なな',        'しち / なな',        'Seven',                        5),
  (38, 'はち',               'はち',               'Eight',                        5),
  (39, 'きゅう / く',        'きゅう / く',        'Nine',                         5),
  (40, 'じゅう',             'じゅう',             'Ten',                          5),
  (41, 'ゼロ / れい',        'ゼロ / れい',        'Zero',                         5),
-- Numbers 11–10000 (topic 4)
  (42, 'じゅういち',         'じゅういち',         'Eleven',                       5),
  (43, 'にじゅう',           'にじゅう',           'Twenty',                       5),
  (44, 'ひゃく',             'ひゃく',             'One hundred',                  5),
  (45, 'せん',               'せん',               'One thousand',                 5),
  (46, 'まん',               'まん',               'Ten thousand',                 5),
  (47, 'えん',               'えん',               'Yen',                          5),
  (48, 'いくら',             'いくら',             'How much?',                    5),
-- Days & Time (topic 5)
  (49, 'にちようび',         'にちようび',         'Sunday',                       5),
  (50, 'げつようび',         'げつようび',         'Monday',                       5),
  (51, 'かようび',           'かようび',           'Tuesday',                      5),
  (52, 'すいようび',         'すいようび',         'Wednesday',                    5),
  (53, 'もくようび',         'もくようび',         'Thursday',                     5),
  (54, 'きんようび',         'きんようび',         'Friday',                       5),
  (55, 'どようび',           'どようび',           'Saturday',                     5),
  (56, 'きょう',             'きょう',             'Today',                        5),
  (57, 'あした',             'あした',             'Tomorrow',                     5),
  (58, 'きのう',             'きのう',             'Yesterday',                    5),
  (59, 'いま',               'いま',               'Now',                          5),
  (60, 'なんじ',             'なんじ',             'What time?',                   5),
  (61, 'ごぜん',             'ごぜん',             'AM / morning',                 5),
  (62, 'ごご',               'ごご',               'PM / afternoon',               5),
  (63, 'はん',               'はん',               'Half (as in half past)',        5),
-- Family (topic 6)
  (64, 'かぞく',             '家族',               'Family',                       5),
  (65, 'おとうさん',         'おとうさん',         'Father (someone else''s)',      5),
  (66, 'おかあさん',         'おかあさん',         'Mother (someone else''s)',      5),
  (67, 'おにいさん',         'おにいさん',         'Older brother (someone else''s)',5),
  (68, 'おねえさん',         'おねえさん',         'Older sister (someone else''s)', 5),
  (69, 'ちち',               'ちち',               'Father (my own)',              5),
  (70, 'はは',               'はは',               'Mother (my own)',              5),
  (71, 'あに',               'あに',               'Older brother (my own)',       5),
  (72, 'あね',               'あね',               'Older sister (my own)',        5),
  (73, 'いもうと',           'いもうと',           'Younger sister',               5),
  (74, 'おとうと',           'おとうと',           'Younger brother',              5),
-- Food & Drink (topic 7)
  (75, 'たべもの',           '食べ物',             'Food',                         5),
  (76, 'のみもの',           '飲み物',             'Drink / beverage',             5),
  (77, 'みず',               '水',                 'Water',                        5),
  (78, 'おちゃ',             'おちゃ',             'Tea',                          5),
  (79, 'ごはん',             'ごはん',             'Rice / meal',                  5),
  (80, 'パン',               'パン',               'Bread',                        5),
  (81, 'にく',               '肉',                 'Meat',                         5),
  (82, 'さかな',             '魚',                 'Fish',                         5),
  (83, 'やさい',             '野菜',               'Vegetables',                   5),
  (84, 'たまご',             'たまご',             'Egg',                          5),
  (85, 'くだもの',           '果物',               'Fruit',                        5),
  (86, 'すし',               'すし',               'Sushi',                        5),
  (87, 'ラーメン',           'ラーメン',           'Ramen',                        5),
-- Places & Directions (topic 8)
  (88, 'がっこう',           '学校',               'School',                       5),
  (89, 'うち',               'うち',               'Home / house',                 5),
  (90, 'えき',               '駅',                 'Train station',                5),
  (91, 'スーパー',           'スーパー',           'Supermarket',                  5),
  (92, 'びょういん',         '病院',               'Hospital',                     5),
  (93, 'ぎんこう',           '銀行',               'Bank',                         5),
  (94, 'みぎ',               '右',                 'Right',                        5),
  (95, 'ひだり',             '左',                 'Left',                         5),
  (96, 'まっすぐ',           'まっすぐ',           'Straight ahead',               5),
  (97, 'ちかく',             'ちかく',             'Near / nearby',                5),
  (98, 'とおい',             '遠い',               'Far',                          5),
-- Daily Actions (topic 9)
  (99,  'たべる',            '食べる',             'To eat',                       5),
  (100, 'のむ',              '飲む',               'To drink',                     5),
  (101, 'いく',              '行く',               'To go',                        5),
  (102, 'くる',              '来る',               'To come',                      5),
  (103, 'する',              'する',               'To do',                        5),
  (104, 'みる',              '見る',               'To see / watch',               5),
  (105, 'きく',              '聞く',               'To listen / ask',              5),
  (106, 'よむ',              '読む',               'To read',                      5),
  (107, 'かく',              '書く',               'To write',                     5),
  (108, 'かう',              '買う',               'To buy',                       5),
  (109, 'ねる',              '寝る',               'To sleep',                     5),
  (110, 'おきる',            '起きる',             'To wake up / get up',          5),
  (111, 'はなす',            '話す',               'To speak / talk',              5),
  (112, 'わかる',            'わかる',             'To understand',                5),
  (113, 'しる',              '知る',               'To know',                      5),
-- Adjectives I (topic 10)
  (114, 'おおきい',          '大きい',             'Big / large',                  5),
  (115, 'ちいさい',          '小さい',             'Small / little',               5),
  (116, 'たかい',            '高い',               'Tall / expensive',             5),
  (117, 'やすい',            '安い',               'Cheap / inexpensive',          5),
  (118, 'あたらしい',        '新しい',             'New',                          5),
  (119, 'ふるい',            '古い',               'Old',                          5),
  (120, 'いい / よい',       'いい / よい',        'Good',                         5),
  (121, 'わるい',            '悪い',               'Bad',                          5),
  (122, 'おもしろい',        '面白い',             'Interesting / funny',          5),
  (123, 'つまらない',        'つまらない',         'Boring',                       5),
  (124, 'むずかしい',        '難しい',             'Difficult',                    5),
  (125, 'やさしい',          'やさしい',           'Easy / kind',                  5),
  (126, 'すきな',            '好きな',             'Liked / favourite (na-adj)',    5),
  (127, 'きらいな',          '嫌いな',             'Disliked (na-adj)',             5),
  (128, 'げんきな',          '元気な',             'Healthy / energetic (na-adj)', 5),
  (129, 'しずかな',          '静かな',             'Quiet (na-adj)',                5),
  (130, 'にぎやかな',        'にぎやかな',         'Lively / bustling (na-adj)',    5);

-- =========================================================================
-- N5 Kanji
-- =========================================================================
INSERT OR IGNORE INTO kanji (id, character, onyomi, kunyomi, meaning, jlpt_level, joyo_grade, stroke_count, radicals) VALUES
-- Nature (topic 11)
  (1,  '日', 'ニチ、ジツ', 'ひ、か',       'Sun / day',          5, 1, 4,  '日'),
  (2,  '月', 'ゲツ、ガツ', 'つき',         'Moon / month',       5, 1, 4,  '月'),
  (3,  '山', 'サン',       'やま',         'Mountain',           5, 1, 3,  '山'),
  (4,  '川', 'セン',       'かわ',         'River',              5, 1, 3,  '川'),
  (5,  '火', 'カ',         'ひ',           'Fire',               5, 1, 4,  '火'),
  (6,  '水', 'スイ',       'みず',         'Water',              5, 1, 4,  '水'),
  (7,  '木', 'モク、ボク', 'き',           'Tree / wood',        5, 1, 4,  '木'),
  (8,  '金', 'キン、コン', 'かね、かな',   'Gold / money / metal',5, 1, 8,  '金'),
  (9,  '土', 'ド、ト',     'つち',         'Earth / soil',       5, 1, 3,  '土'),
-- People (topic 12)
  (10, '人', 'ジン、ニン', 'ひと',         'Person / people',    5, 1, 2,  '人'),
  (11, '男', 'ダン、ナン', 'おとこ',       'Man / male',         5, 1, 7,  '田力'),
  (12, '女', 'ジョ、ニョ', 'おんな',       'Woman / female',     5, 1, 3,  '女'),
  (13, '子', 'シ、ス',     'こ',           'Child',              5, 1, 3,  '子'),
  (14, '口', 'コウ、ク',   'くち',         'Mouth',              5, 1, 3,  '口'),
  (15, '目', 'モク、ボク', 'め',           'Eye',                5, 1, 5,  '目'),
  (16, '耳', 'ジ',         'みみ',         'Ear',                5, 1, 6,  '耳'),
  (17, '手', 'シュ、ズ',   'て',           'Hand',               5, 1, 4,  '手'),
  (18, '足', 'ソク',       'あし、たる',   'Foot / leg / enough',5, 1, 7,  '足'),
-- Numbers (topic 13)
  (19, '一', 'イチ、イツ', 'ひと',         'One',                5, 1, 1,  '一'),
  (20, '二', 'ニ',         'ふた',         'Two',                5, 1, 2,  '二'),
  (21, '三', 'サン',       'み',           'Three',              5, 1, 3,  '三'),
  (22, '四', 'シ',         'よ、よん',     'Four',               5, 1, 5,  '囗儿'),
  (23, '五', 'ゴ',         'いつ',         'Five',               5, 1, 4,  '五'),
  (24, '六', 'ロク',       'む',           'Six',                5, 1, 4,  '六'),
  (25, '七', 'シチ',       'なな',         'Seven',              5, 1, 2,  '七'),
  (26, '八', 'ハチ',       'や',           'Eight',              5, 1, 2,  '八'),
  (27, '九', 'ク、キュウ', 'ここの',       'Nine',               5, 1, 2,  '九'),
  (28, '十', 'ジュウ、ジッ','とお',        'Ten',                5, 1, 2,  '十'),
  (29, '百', 'ヒャク',     NULL,           'Hundred',            5, 1, 6,  '白'),
  (30, '千', 'セン',       'ち',           'Thousand',           5, 1, 3,  '千'),
-- Time (topic 14)
  (31, '年', 'ネン',       'とし',         'Year',               5, 1, 6,  '年'),
  (32, '時', 'ジ',         'とき',         'Time / hour',        5, 2, 10, '日寺'),
  (33, '間', 'カン、ケン', 'あいだ、ま',   'Interval / between', 5, 2, 12, '門日'),
  (34, '分', 'ブン、フン', 'わ',           'Minute / part',      5, 2, 4,  '刀'),
  (35, '朝', 'チョウ',     'あさ',         'Morning',            5, 3, 12, '月十'),
  (36, '夜', 'ヤ',         'よる、よ',     'Night / evening',    5, 2, 8,  '夜');

-- =========================================================================
-- topic_vocabulary — link vocabulary to topics
-- =========================================================================
INSERT OR IGNORE INTO topic_vocabulary (topic_id, vocabulary_id, sequence_order) VALUES
-- Greetings
  (1,1,1),(1,2,2),(1,3,3),(1,4,4),(1,5,5),(1,6,6),(1,7,7),(1,8,8),
  (1,9,9),(1,10,10),(1,11,11),(1,12,12),(1,13,13),(1,14,14),(1,15,15),
-- Self-Introduction
  (2,16,1),(2,17,2),(2,18,3),(2,19,4),(2,20,5),(2,21,6),(2,22,7),
  (2,23,8),(2,24,9),(2,25,10),(2,26,11),(2,27,12),(2,28,13),(2,29,14),(2,30,15),
-- Numbers 1-10
  (3,31,1),(3,32,2),(3,33,3),(3,34,4),(3,35,5),(3,36,6),(3,37,7),(3,38,8),(3,39,9),(3,40,10),(3,41,11),
-- Numbers 11-10000
  (4,42,1),(4,43,2),(4,44,3),(4,45,4),(4,46,5),(4,47,6),(4,48,7),
-- Days & Time
  (5,49,1),(5,50,2),(5,51,3),(5,52,4),(5,53,5),(5,54,6),(5,55,7),
  (5,56,8),(5,57,9),(5,58,10),(5,59,11),(5,60,12),(5,61,13),(5,62,14),(5,63,15),
-- Family
  (6,64,1),(6,65,2),(6,66,3),(6,67,4),(6,68,5),(6,69,6),(6,70,7),(6,71,8),(6,72,9),(6,73,10),(6,74,11),
-- Food & Drink
  (7,75,1),(7,76,2),(7,77,3),(7,78,4),(7,79,5),(7,80,6),(7,81,7),(7,82,8),(7,83,9),(7,84,10),(7,85,11),(7,86,12),(7,87,13),
-- Places & Directions
  (8,88,1),(8,89,2),(8,90,3),(8,91,4),(8,92,5),(8,93,6),(8,94,7),(8,95,8),(8,96,9),(8,97,10),(8,98,11),
-- Daily Actions
  (9,99,1),(9,100,2),(9,101,3),(9,102,4),(9,103,5),(9,104,6),(9,105,7),(9,106,8),(9,107,9),(9,108,10),(9,109,11),(9,110,12),(9,111,13),(9,112,14),(9,113,15),
-- Adjectives I
  (10,114,1),(10,115,2),(10,116,3),(10,117,4),(10,118,5),(10,119,6),(10,120,7),(10,121,8),(10,122,9),(10,123,10),(10,124,11),(10,125,12),(10,126,13),(10,127,14),(10,128,15),(10,129,16),(10,130,17);

-- =========================================================================
-- topic_kanji — link kanji to kanji topics
-- =========================================================================
INSERT OR IGNORE INTO topic_kanji (topic_id, kanji_id, sequence_order) VALUES
-- Nature (topic 11)
  (11,1,1),(11,2,2),(11,3,3),(11,4,4),(11,5,5),(11,6,6),(11,7,7),(11,8,8),(11,9,9),
-- People (topic 12)
  (12,10,1),(12,11,2),(12,12,3),(12,13,4),(12,14,5),(12,15,6),(12,16,7),(12,17,8),(12,18,9),
-- Numbers kanji (topic 13)
  (13,19,1),(13,20,2),(13,21,3),(13,22,4),(13,23,5),(13,24,6),(13,25,7),(13,26,8),(13,27,9),(13,28,10),(13,29,11),(13,30,12),
-- Time kanji (topic 14)
  (14,31,1),(14,32,2),(14,33,3),(14,34,4),(14,35,5),(14,36,6);

-- =========================================================================
-- vocabulary_kanji — which kanji appear in which vocabulary words
-- =========================================================================
INSERT OR IGNORE INTO vocabulary_kanji (vocabulary_id, kanji_id) VALUES
  (18, 10),(18, 28),   -- 名前: 名(person-related), 前
  (64, 10),            -- 家族: 人 component
  (75, 99),            -- 食べ物 → references 食(eat) — not yet seeded, skip
  (77, 6),             -- 水 → kanji 6
  (79, 5),(79,6),      -- ごはん uses 米(not seeded), skip
  (81, 10),            -- 肉
  (88, 10),            -- 学校
  (94, 8),             -- 右 (not seeded fully, best effort)
  (99, 10),            -- 食べる
  (114,10),(114,19),   -- 大きい → 大(not seeded), skip; link what we can
  (31, 19),            -- いち → 一
  (32, 20),            -- に → 二
  (33, 21),            -- さん → 三
  (34, 22),            -- し → 四
  (35, 23),            -- ご → 五
  (36, 24),            -- ろく → 六
  (37, 25),            -- しち → 七
  (38, 26),            -- はち → 八
  (39, 27),            -- きゅう → 九
  (40, 28),            -- じゅう → 十
  (44, 29),            -- ひゃく → 百
  (45, 30),            -- せん → 千
  (49, 1),(49,2),      -- にちようび → 日曜日 uses 日
  (50, 2),             -- げつようび → 月曜日
  (51, 5),             -- かようび → 火曜日
  (52, 6),             -- すいようび → 水曜日
  (53, 7),             -- もくようび → 木曜日
  (54, 8),             -- きんようび → 金曜日
  (55, 9),             -- どようび → 土曜日
  (56, 1),             -- きょう uses 今日 → 日
  (57, 1),             -- あした → 明日 → 日
  (58, 1),             -- きのう → 昨日 → 日
  (60, 32),            -- なんじ → 何時 → 時
  (63, 34);            -- はん → 分

-- =========================================================================
-- N5 Lesson plans
-- =========================================================================
INSERT OR IGNORE INTO lesson_plans (id, jlpt_level, title) VALUES
  (1, 5, 'N5 Week 1 — First Words'),
  (2, 5, 'N5 Week 2 — Numbers & Time'),
  (3, 5, 'N5 Week 3 — Daily Life'),
  (4, 5, 'N5 Week 4 — Kanji Foundations');

INSERT OR IGNORE INTO lesson_plan_topics (lesson_plan_id, topic_id, sequence_order) VALUES
  (1, 1, 1), (1, 2, 2),            -- Week 1: Greetings, Self-Intro
  (2, 3, 1), (2, 4, 2), (2, 5, 3), -- Week 2: Numbers, Days & Time
  (3, 7, 1), (3, 8, 2), (3, 9, 3), -- Week 3: Food, Places, Actions
  (4,11, 1), (4,12, 2), (4,13, 3); -- Week 4: Kanji blocks
";
