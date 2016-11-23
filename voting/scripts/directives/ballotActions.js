(function() {
    'use strict';

    angular
        .module('votingApp')
        .directive('ballotActions', ballotActions);

    /* @ngInject */
    function ballotActions($timeout, $state) {
        return {
            restrict: 'EA',
            templateUrl: 'partials/directives/ballotActions.html',
            link: function(scope) {
                var checkPromise = undefined;

                scope.numbersEntered = 0;

                scope.keyboardButtonClick = function() {
                    scope.numbersEntered++;
                    if (scope.numbersEntered > 4) {
                        if (checkPromise) {
                            $timeout.cancel(checkPromise);
                        }
                        checkPromise = $timeout(function() {
                            scope.numbersEntered = 0;
                        }, 2000);
                    }

                };

                scope.decryptModal = function() {
                    $('#decryptModal').modal();
                };

                scope.submitDecrypt = function() {
                    $('#decryptModal').modal('hide');
                    $('.app-wrapper').scrollTop(0);
                    $state.go('decrypted');
                };

                scope.signModal = function() {
                    $('#signModal').modal();
                };

                scope.submitSign = function() {
                    $('#signModal').modal('hide');
                    $('.app-wrapper').scrollTop(0);
                    $state.go('signed');
                };
            }
        };
    }
})();