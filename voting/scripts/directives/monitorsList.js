(function() {
    'use strict';

    angular
        .module('votingApp')
        .directive('monitorsList', monitorsList);

    /* @ngInject */
    function monitorsList(constants) {
        return {
            restrict: 'EA',
            templateUrl: 'partials/directives/monitorsList.html',
            link: function(scope, element) {
                scope.monitors = constants.monitors;

                scope.toggle = function(current) {
                    current.active = !current.active;
                };
            }
        };
    }
})();