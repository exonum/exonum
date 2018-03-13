# Frontend Tutorial

We are going to build application on [Vue.js](https://vuejs.org)
framework together with [Bootstrap](https://getbootstrap.com/).

First, create skeleton for future application.

Create application template `src/App.vue`:

```html
<template>
  <div>
    <router-view/>
  </div>
</template>

```

Define application `src/app.js`:

```javascript
import Vue from 'vue'
import router from './router'
import App from './App.vue'

new Vue({
  el: '#app',
  router,
  render: (createElement) => createElement(App)
})
```

Define router `src/router/index.js`:

```javascript
import Vue from 'vue'
import Router from 'vue-router'
import AuthPage from '../pages/AuthPage.vue'

Vue.use(Router)

export default new Router({
  routes: [
    {
      path: '/',
      name: 'home',
      component: AuthPage
    }
  ]
})
```

Mount application into `index.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta charset="UTF-8">
    <title>Cryptocurrency</title>

    <link rel="shortcut icon" href="favicon.ico" type="image/x-icon">
    <link rel="stylesheet" href="node_modules/bootstrap/dist/css/bootstrap.min.css">
</head>
<body>

<div id="app"></div>
<script src="dist/build.js"></script>

</body>
</html>
```

Write [Grunt](https://gruntjs.com/) script `Gruntfile.js` to compile Vue.js:

```javascript
module.exports = function(grunt) {
    require('load-grunt-tasks')(grunt);

    grunt.initConfig({
        browserify: {
            options: {
                transform: [
                    ['vueify'],
                    ['babelify', {presets: 'es2015'}]
                ]
            },
            dist: {
                src: './src/app.js',
                dest: './dist/build.js'
            }
        }
    });
};
```

Next step is to create [registration form](registration-form.md).
