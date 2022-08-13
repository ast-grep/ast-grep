<script lang="ts">
</script>

<script lang="ts" setup>
import * as monaco from 'monaco-editor';
import {ref, onMounted, onBeforeUnmount, shallowRef} from 'vue'

const emits = defineEmits<{
    (e: 'change', value: string): void,
}>()

const props = defineProps({
  language: {
    type: String,
    default: 'javascript'
  },
  readonly: {
    type: Boolean,
    default: false,
  }
})


const containerRef = ref<HTMLDivElement | null>(null)
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null);

onMounted(() => {
  if (!containerRef.value) {
    return
  }
  const editorInstance = monaco.editor.create(containerRef.value, {
    value: '',
    language: props.language,
    readOnly: props.readonly,
    automaticLayout: true,
    scrollBeyondLastLine: false,
    minimap: {
      enabled: false,
    },
    inlineSuggest: {
      enabled: true,
    },
  })
  editor.value = editorInstance
  editorInstance.onDidChangeModelContent(() => {
      emits('change', editorInstance.getValue())
  })
})

onBeforeUnmount(() => {
  editor.value?.dispose()
})

</script>

<template>
  <div class="editor" ref="containerRef"/>
</template>

<style scoped>
.editor {
  width: 100%;
  height: 100%;
  text-align: left;
}
</style>
