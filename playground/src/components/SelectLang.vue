<script lang="ts" setup>

defineProps({
  modelValue: {
    type: String,
    default: 'javascript',
  },
})

const emits = defineEmits<{
  (e: 'update:modelValue', val: string): void
}>()

const languages = {
  'javascript': 'JavaScript',
  'typescript': 'TypeScript',
}

function onChange(event: Event) {
  const target = event.target as HTMLSelectElement
  emits('update:modelValue', String(target.value))
}

</script>
<template>
  <div class="selector">
    Language:
    <select :value="modelValue" @change="onChange">
      <option v-for="val, key in languages" :value="key">{{val}}</option>
    </select>
    </div>
</template>

<style scoped>
.selector {
  text-align: left;
}
select {
  line-height: 1.5em;
  padding: 0.5rem;
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  cursor: pointer;
}
</style>
