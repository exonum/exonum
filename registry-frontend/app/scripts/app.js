'use strict';

/**
 * @ngdoc overview
 * @name landTitleUi1App
 * @description
 * # landTitleUi1App
 *
 * Main module of the application.
 */
angular
  .module('landTitleUi1App', [
    'ngAnimate',
    'ngAria',
    'ngCookies',
    'ngMessages',
    'ngResource',
    'ui.router',
    'ui.bootstrap',
    'ngSanitize',
    'ngTouch',
    'ngMap',
    'ngStorage'
  ])
  .config(function($stateProvider, $urlRouterProvider) {

    $urlRouterProvider.when('', '/');

    $stateProvider
      .state('map', {
        url: '/',
        templateUrl: 'views/map.html',
        controller: 'MapCtrl'
      })
      .state('register', {
        url: '/register',
        templateUrl: 'views/user_register.html',
        controller: 'UserRegisterCtrl',
        data: {
          'noLogin': true
        }
      })
  })
  .run(['$rootScope', '$state', '$stateParams', '$cookies', function($rootScope, $state, $stateParams, $cookies) {
    $rootScope.$on('$stateChangeStart',
      function(event, toState, toParams, fromState, fromParams) {
        if (toState.name != 'register' && !$cookies.get('public_key')) {
          console.log('needs to register');
          event.preventDefault();
          $state.go('register');
        }
      }
    );

  }]);
