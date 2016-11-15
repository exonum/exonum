'use strict';

angular.module('landTitleUi1App')
.controller('UserRegisterCtrl', function ($scope, $state, $api) {
    $scope.register = function (){
        $api.registerUser({ name: $scope.name }).then(function (success){
            $state.go('map');
        }, function (error){
            console && console.log(error);
            $state.go('register');
        });

    }
});