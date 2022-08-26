<script lang="ts">
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker';
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker';
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';
// LOL vue sfc compiler does not allow type alias and dynamic import co-exist
import type monaco from 'monaco-editor'

// @ts-ignore
self.MonacoEnvironment = {
	getWorker(_: any, label: string) {
		if (label === 'json') {
			return new jsonWorker();
		}
		if (label === 'css' || label === 'scss' || label === 'less') {
			return new cssWorker();
		}
		if (label === 'html' || label === 'handlebars' || label === 'razor') {
			return new htmlWorker();
		}
		if (label === 'typescript' || label === 'javascript') {
			return new tsWorker();
		}
		return new editorWorker();
	}
};

</script>

<script lang="ts" setup>
import {
  ref,
  onMounted,
  onBeforeUnmount,
  shallowRef,
  watch,
  PropType,
} from 'vue'

const emits = defineEmits<{
    (e: 'update:modelValue', value: string): void,
}>()

const props = defineProps({
  language: {
    type: String,
    default: 'javascript'
  },
  modelValue: String,
  readonly: {
    type: Boolean,
    default: false,
  },
  highlights: {
    type: Array as PropType<number[][]>,
  },
})

const monaco = await import('monaco-editor')

const containerRef = ref<HTMLDivElement | null>(null)
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null);

let highlights: monaco.editor.IEditorDecorationsCollection | null = null

onMounted(() => {
  if (!containerRef.value) {
    return
  }
  const editorInstance = monaco.editor.create(containerRef.value, {
    value: props.modelValue,
    language: props.language,
    readOnly: props.readonly,
    automaticLayout: false,
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
      emits('update:modelValue', editorInstance.getValue())
  })
  highlights = editorInstance.createDecorationsCollection(props.highlights?.map(transformMatch) || [])
})

const transformMatch = (match: number[]) => {
    const [sr, sc, er, ec] = match
    return {
      range: new monaco.Range(sr + 1, sc + 1, er + 1, ec + 1),
      options: {
        inlineClassName: 'monaco-highlight-span'
      }
    }
}

watch(() => props.highlights, (matched) => {
  const ranges = matched!.map(transformMatch)
  highlights?.set(ranges)
})

watch(() => props.language, (lang) => {
  let oldModel = editor.value?.getModel()
  let newModel = monaco.editor.createModel(props.modelValue || '', lang)
  editor.value?.setModel(newModel)
  if (oldModel) {
    oldModel.dispose()
  }
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
  border: 1px solid #eee;
}
</style>
<style>
.monaco-highlight-span {
  background-color: var(--theme-highlight3);
}
</style>
