<script setup lang="ts">
import { ref } from 'vue';
const count = ref(0);
function bump() { count.value++; }
</script>
<template>
  <div>
    <my-button @click="bump" />
    <Modal :visible="true" />
    <Transition name="fade"><p>hi</p></Transition>
    <span>{{ count }}</span>
  </div>
</template>
