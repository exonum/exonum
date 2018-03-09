<template>
    <div class="tab-pane" v-bind:class="{ 'active': active }">
        <slot></slot>
    </div>
</template>

<script>
    module.exports = {
        name: 'tab',
        data: function() {
            return {
                active: false
            };
        },
        props: {
            title: String,
            isActive: {
                required: false,
                type: Boolean,
                default: false
            }
        },
        created: function() {
            this.active = this.isActive;
        },
        mounted: function() {
            this.$parent.addTab(this);
        }
    };
</script>
