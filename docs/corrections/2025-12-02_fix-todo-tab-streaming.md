# Rapport de Correction - Todo Tab Streaming

## Metadonnees
- **Date**: 2025-12-02
- **Erreurs initiales**: 1 (bug logique)
- **Erreurs corrigees**: 1
- **Nouvelles regressions**: 0

## Diagnostic Initial

### Probleme Signale
Les tâches dans l'onglet "Todo" de la sidebar Activity ne s'affichent qu'à la fin de l'exécution du workflow, alors qu'elles devraient apparaître en temps réel dès leur création.

### Analyse Root Cause

**Localisation du bug**: `src/lib/stores/streaming.ts:368-378`

**Cause racine identifiée**: Les gestionnaires d'événements de tâches (`addTask`, `updateTaskStatus`, `completeTask`) étaient implémentés comme des fonctions séparées qui appelaient `store.update()` de manière imbriquée à l'intérieur du `store.update()` principal dans `processChunk()`. Bien que les appels imbriqués fonctionnent, le `return s` retournait l'état **original** au lieu de l'état mis à jour par les fonctions.

**Code problématique**:
```typescript
case 'task_create':
    addTask(chunk);      // Appelle store.update() en interne
    return s;            // BUG: Retourne l'état original, pas l'état mis à jour

case 'task_update':
    updateTaskStatus(chunk);
    return s;            // Même bug

case 'task_complete':
    completeTask(chunk);
    return s;            // Même bug
```

**Pourquoi les autres événements fonctionnent**:
Les événements `tool_start`, `tool_end`, `reasoning`, etc. font leurs mises à jour **inline** dans la fonction `processChunk`, retournant directement le nouvel état.

## Corrections Appliquees

### Correction du pattern dans streaming.ts

**Solution**: Réécrire les handlers de tâches en inline, comme les autres types d'événements.

```typescript
case 'task_create':
    return {
        ...s,
        tasks: [
            ...s.tasks,
            {
                id: chunk.task_id!,
                name: chunk.task_name!,
                status: (chunk.task_status ?? 'pending') as ActiveTask['status'],
                priority: chunk.task_priority ?? 3,
                createdAt: Date.now(),
                updatedAt: Date.now()
            }
        ]
    };

case 'task_update':
    return {
        ...s,
        tasks: s.tasks.map((t) =>
            t.id === chunk.task_id
                ? { ...t, status: chunk.task_status as ActiveTask['status'], updatedAt: Date.now() }
                : t
        )
    };

case 'task_complete':
    return {
        ...s,
        tasks: s.tasks.map((t) =>
            t.id === chunk.task_id
                ? { ...t, status: 'completed' as const, updatedAt: Date.now() }
                : t
        )
    };
```

### Nettoyage
Les fonctions `addTask`, `updateTaskStatus`, `completeTask` (lignes 182-236) ont été supprimées car elles ne sont plus utilisées.

## Validation Finale

### Frontend
- **Lint**: PASS (0 erreurs, 3 warnings dans fichiers générés i18n)
- **TypeCheck**: PASS (0 erreurs)

### Backend
- **Clippy**: PASS (0 warnings)

## Prevention

### Pattern a Eviter
Ne JAMAIS appeler `store.update()` de manière imbriquée et retourner l'état d'entrée:

```typescript
// MAUVAIS - Ne pas faire
function processChunk(chunk: StreamChunk): void {
    store.update((s) => {
        switch (chunk.type) {
            case 'some_event':
                someHelperThatCallsStoreUpdate(chunk);  // Modifie le store
                return s;  // Retourne l'ancien état!
        }
    });
}

// BON - Faire les modifications inline
function processChunk(chunk: StreamChunk): void {
    store.update((s) => {
        switch (chunk.type) {
            case 'some_event':
                return { ...s, field: newValue };  // Retourne le nouvel état
        }
    });
}
```

## Lecons Apprises

1. Dans Svelte stores, `store.update()` doit retourner le nouvel état - ne pas déléguer à des fonctions qui font leur propre `store.update()`
2. Maintenir la cohérence des patterns: si la plupart des cas font des updates inline, les nouveaux cas devraient suivre le même pattern
3. Tester le streaming en temps réel, pas seulement le résultat final
