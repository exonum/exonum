<template>
    <div>
        <ul class="nav nav-tabs mb-4">
            <li class="nav-item" v-for="tab in tabs">
                <a href="#" class="nav-link" v-bind:class="{ 'active': current === tab }" v-on:click.prevent="changeTab(tab)">
                    {{ tab.title }}
                </a>
            </li>
        </ul>

        <div class="tab-content">
            <slot v-on:mount="addTab"></slot>
        </div>
    </div>
</template>

<script>
    module.exports = {
        name: 'tabs',
        data: function() {
            return {
                tabs: [],
                current: null
            }
        },
        methods: {
            addTab: function(tab) {
                this.tabs.push(tab);
                if (tab.active === true) {
                    this.current = tab;
                }
            },
            changeTab: function(tab) {
                this.current = tab;
                this.tabs.forEach(function(value) {
                    value.active = value === tab;
                });
            }
        }
    };
</script>