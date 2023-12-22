<template>
  <div>
    <h1>{{ greeting }}</h1>
    <p>{{ message }}</p>
  </div>
</template>

<script>
export default {
  data() {
    return {
      greeting: "Hello",
      message: "Welcome to Vue SFC!",
    };
  },
};
</script>

<style>
h1 {
  color: #42b983;
}
</style>