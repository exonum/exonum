(function() {
    'use strict';

    angular
        .module('votingApp')
        .directive('electionsList', electionsList);

    /* @ngInject */
    function electionsList(constants, $rootScope) {
        return {
            restrict: 'EA',
            templateUrl: 'partials/directives/electionsList.html',
            link: function(scope, element) {
                scope.toggle = function(current) {
                    $rootScope.currentElection = current;
                    $rootScope.currentCandidate = $rootScope.currentElection.candidates[0];
                    $rootScope.setDataIndex();
                };
            }
        };
    }
})();