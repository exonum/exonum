<template>
  <div>
    <ul class="nav nav-tabs mb-4">
      <!-- eslint-disable-next-line vue/require-v-for-key -->
      <li v-for="tab in tabs" class="nav-item">
        <a :class="{ 'active': current === tab }" href="#" class="nav-link" @click.prevent="changeTab(tab)">
          {{ tab.title }}
        </a>
      </li>
    </ul>

    <div class="tab-content">
      <slot @mount="addTab"/>
    </div>
  </div>
</template>

<script>
  module.exports = {
    name: 'tabs',
    data() {
      return {
        tabs: [],
        current: null
      }
    },
    methods: {
      addTab(tab) {
        this.tabs.push(tab)
        if (tab.active === true) {
          this.current = tab
        }
      },

      changeTab(tab) {
        this.current = tab
        this.tabs.forEach(value => {
          value.active = value === tab
        })
      }
    }
  }
</script>
