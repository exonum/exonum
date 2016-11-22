(function() {
    'use strict';

    angular
        .module('votingApp')
        .directive('candidatesList', candidatesList);

    /* @ngInject */
    function candidatesList(constants, $state, $rootScope) {
        return {
            restrict: 'EA',
            templateUrl: 'partials/directives/candidatesList.html',
            link: function(scope, element) {
                scope.toggle = function(current) {
                    $rootScope.currentCandidate = current;
                    $rootScope.setDataIndex();
                };

                scope.chooseCandidate = function() {
                    $('#candidateModal').modal();
                };

                scope.submitCandidate = function() {
                    $('#candidateModal').modal('hide');
                    $('.app-wrapper').scrollTop(0);
                    $state.go('ballot');
                };
            }
        };
    }
})();