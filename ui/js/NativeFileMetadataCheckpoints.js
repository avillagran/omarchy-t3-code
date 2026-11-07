.pragma library
// Native checkpoint invalidation effect 32a0531748c44b25072d7495ce0b9bd7e9450a87.
.import "NativeFileMetadata.js" as Paths
function create(){const previous={current:null};const resolveWorkspaceFilePath=Paths.create().resolveWorkspace;
function update(threadRef,cwd,checkpoints,invalidate){
    if (!threadRef || !cwd || !checkpoints) {
      previous.current = null;
      return;
    }
    const before = previous.current;
    previous.current = Object.assign({},threadRef,{cwd:cwd,checkpoints:checkpoints});
    if (
      !before ||
      before.environmentId !== threadRef.environmentId ||
      before.threadId !== threadRef.threadId ||
      before.cwd !== cwd
    )
      return;
    const oldById = new Map(before.checkpoints.map((checkpoint) => [checkpoint.id, checkpoint]));
    const paths = new Set        ();
    for (const checkpoint of checkpoints) {
      const old = oldById.get(checkpoint.id);
      oldById.delete(checkpoint.id);
      if (old?.files === checkpoint.files && old?.status === checkpoint.status) continue;
      for (const file of checkpoint.files) paths.add(resolveWorkspaceFilePath(file.path, cwd));
      for (const file of old?.files ?? []) paths.add(resolveWorkspaceFilePath(file.path, cwd));
    }
    for (const checkpoint of oldById.values())
      for (const file of checkpoint.files) paths.add(resolveWorkspaceFilePath(file.path, cwd));
    if (paths.size > 0)
      void invalidate({ environmentId: threadRef.environmentId, input: { paths: Array.from(paths) } });
}
return {update:update};
}
