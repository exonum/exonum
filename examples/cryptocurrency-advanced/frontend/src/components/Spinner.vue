<template>
  <div v-if="visible" class="spinner"/>
</template>

<script>
  module.exports = {
    name: 'spinner',
    props: {
      visible: Boolean
    }
  }
</script>

<style>
  .spinner {
    height: 100%;
    left: 0;
    position: fixed;
    top: 0;
    width: 100%;
    z-index: 10000;
  }

  .spinner:before {
    background-color: rgba(255, 255, 255, .9);
    content: '';
    display: block;
    height: 100%;
    left: 0;
    position: absolute;
    top: 0;
    width: 100%;
  }

  .spinner:after {
    background-image: url(images/spinner.gif);
    background-size: 100%;
    content: '';
    display: block;
    height: 60px;
    left: 50%;
    margin: -30px 0 0 -30px;
    position: absolute;
    top: 50%;
    width: 60px;
  }
</style>
