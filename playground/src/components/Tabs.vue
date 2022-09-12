<script setup lang="ts">
import { ref, defineProps, defineEmits, PropType, h } from 'vue'

const props = defineProps({
  modelValue: String,
  modeText: Object as PropType<Record<string, string>>,
})

const emits = defineEmits<{
  (e: 'update:modelValue', value: string): void,
}>()

function switchTab(m: string) {
  emits('update:modelValue', m)
}

const Wrapper = {
  render() {
    return h('div', this.$attrs, this.$slots.default())
  }
}

</script>

<template>
  <div class="tabs">
    <a
      class="tab"
      :class="{active: modelValue === mode}"
      v-for="tabText, mode in modeText"
      @click="switchTab(mode)"
    >
      {{ tabText }}
    </a>
    <div class="addon">
      <slot name="addon"/>
    </div>
  </div>
  <div class="tab-content">
      <template  v-for="_, mode in modeText">
        <transition>
          <keep-alive>
            <Wrapper class="content-wrapper" v-if="modelValue === mode">
              <slot :name="mode"/>
            </Wrapper>
          </keep-alive>
        </transition>
      </template>
  </div>
</template>

<style scoped>

.tabs {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  margin-bottom: -1px;
}

.tab {
  margin-right: 0.2em;
  border-radius: 5px 5px 0 0;
  border: 1px solid #eee;
  padding: 8px 16px;
  cursor: pointer;
  z-index: 2;
  color: inherit;
  opacity: 0.75;
  position: relative;
  top: 0;
  transition: top linear 0.16s;
}

.tab.active {
  background-color: #fff; /* required for drop-shadow*/
  border-bottom-color: #fff;
  opacity: 1;
  color: var(--brand-color);
}
.tab-content {
  flex: 1 0 auto;
  background: white;
  border: 1px solid #eee;
  border-radius: 0 5px 5px 5px;
  overflow: hidden;
  position: relative;
}
.content-wrapper {
  top: 0;
  bottom: 0;
  left: 0;
  right: 0;
  position: absolute;
  transition: all ease-in-out 0.3s;
}
.v-enter-to,
.v-leave-from {
  opacity: 1;
  transform: translateY(0);
}

.v-enter-from,
.v-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.addon {
  margin-left: auto;
}
</style>
