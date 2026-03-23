# Class Lifecycle Refactor

## 旧モデル

従来はクラス関連の状態が複数の `HashMap<String, ...>` に分散していた。

- `classes`: 生バイト / JAR entry / parse 済み `ClassFile`
- `class_init_states`: `<clinit>` の実行状態
- `class_runtime_metadata_cache`: メソッド探索と `<clinit>` 判定用の準備済みメタデータ
- `class_init_prerequisite_cache`: 親クラス / superinterface 初期化順

この構造では、同じクラス名に対して

- どこで identity lookup をしているか
- どこで parse / prepare / init を進めているか
- どこで sticky failure を見ているか

が入口ごとにばらけやすく、`Class.forName(...)`、静的アクセス、反射、`ClassLoader` API の経路が揃っていなかった。

## 新モデル

クラスを `ClassRecord` として一元管理する。

- `ClassId`: VM 内の安定 ID
- `class_identity_index`: `(defining_loader, binary_name) -> ClassId`
- `class_records[ClassId]`: source / lifecycle / prepared metadata / terminal error

`ClassRecord` は以下を持つ。

- identity: `defining_loader`, `binary_name`
- source: pending bytes / pending jar entry / ready `ClassFile` / parse error
- lifecycle: `Loading | Loaded | Prepared | Initializing | Initialized | Erroneous`
- prepared metadata: `ClassRuntimeMetadata`
- init prerequisites: direct superclass / superinterface 初期化依存

実際の loader 実装はまだ単一の VM 既定 loader を使うが、identity index 自体は defining loader を明示的に持つ。

追加で、hot path の内部キャッシュも `ClassId` ベースへ寄せた。

- method owner / signature / exec info cache
- static field storage
- static field owner cache
- canonical `java/lang/Class` mirror cache
- instance field layout / slot cache
- `instanceof` cache
- heap object runtime class memoization
- `<clinit>` prerequisite / superinterface 順序
- reflection field / method / constructor metadata cache

これにより、入口で一度 identity を引いた後は、super-chain / interface-chain の再帰で class name に戻らない。
registered class の `java/lang/Class` mirror も `ClassId` に紐づき、`Class.forName` / class literal / `Object.getClass()` は同じ mirror に収束する。

## 状態遷移 API

主要な入口は次の内部 API に寄せた。

- `ensure_class_loaded_by_name`
- `ensure_class_prepared`
- `ensure_class_init`

結果として以下が同じ lifecycle を通る。

- `Class.forName0` / `Class.forName1`
- `ClassLoader.loadClass` / `findClass` / `findLoadedClass`
- `getstatic` / `putstatic` / `invokestatic`
- `new`
- 反射系の class metadata 参照

## JVMS との対応

- JVMS 5.3: identity lookup と class derivation は `class_identity_index` と `ClassSource`
- JVMS 5.4: link/preparation 相当は `Loaded -> Prepared`
- JVMS 5.5: `Initializing -> Initialized | Erroneous`

`Erroneous` は terminal error を伴い、初期化失敗は sticky に扱う。

## なぜ速くなるか

- 名前文字列から毎回別 map を辿る代わりに、identity lookup 後は `ClassId` の単一 record を読む
- prepared metadata を record に保持し、`<clinit>` 依存解決を再構築しない
- method / field / subtype / instance-layout の内部再帰を `ClassId` で閉じ、名前ベース探索を境界に押し込める
- static field storage と reflection metadata cache を class name ではなく `ClassId` で共有し、`getstatic` / `putstatic` / reflection access の owner lookup を繰り返さない
- heap object が runtime `ClassId` を一度 memoize すると、virtual dispatch / `instanceof` / instance field access が object ごとに文字列 lookup を繰り返さない
- `java/lang/Class` mirror が represented `ClassId` を保持するので、`Class.getName` / `getModifiers` / `isAssignableFrom` / `getDeclaredMethods` などが mirror から直接 lifecycle/state に寄れる
- `Class.forName(...)` が load / prepare / init の明示 API を通るので重複判定が減る
- CP member/class 解決をキャッシュし、反復する `getstatic` / `putstatic` / `invokestatic` / class refs の decode を削減する
- profiler reset を使って bundle/JAR 読み込みの一回性コストを除外し、runtime 区間だけの repeated lookup / init check を観測できる

## 今回のスコープ外

- 真の multi-loader namespace 分離
- initiating-loader index
- verifier 完備
- HotSpot 同等の resolution data 構造

ただし今回の `ClassId + ClassRecord + identity index` は、その先の拡張の土台になる。
